use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use toknila_core::{Actor, Timestamp};

use crate::server::{AppState, router};
use crate::store::{Store, StoreError};

async fn emit(store: &Store, n: i64) {
    for i in 0..n {
        store
            .write(
                Actor::System,
                Timestamp(i),
                move |tx| -> Result<(), StoreError> {
                    tx.emit("test", i, "tick", serde_json::json!({ "i": i }))
                },
            )
            .await
            .expect("write");
    }
}

type Ws = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn connect(port: u16, since: i64) -> Ws {
    let url = format!("ws://127.0.0.1:{port}/v1/stream?since={since}&token=tkn");
    let (ws, _) = tungstenite::connect(url).expect("connect");
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(Duration::from_millis(700)))
            .expect("timeout");
    }
    ws
}

/// Reads seqs until the socket is quiet.
fn read_seqs(ws: &mut Ws, want: usize) -> Vec<i64> {
    let mut out = Vec::new();
    while out.len() < want + 1 {
        match ws.read() {
            Ok(tungstenite::Message::Text(t)) => {
                let v: serde_json::Value = serde_json::from_str(t.as_str()).expect("json");
                out.push(v["seq"].as_i64().expect("seq"));
            }
            Ok(_) => {}
            Err(_) => break, // timeout: quiet
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnect_gets_exactly_the_missed_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("t.db")).expect("store");
    let state = AppState {
        store: store.clone(),
        token: "tkn".into(),
        shutdown: Arc::new(Notify::new()),
    };
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move { axum::serve(listener, router(state)).await });

    emit(&store, 5).await;
    let mut ws = tokio::task::spawn_blocking(move || {
        let mut ws = connect(port, 0);
        assert_eq!(read_seqs(&mut ws, 5), vec![1, 2, 3, 4, 5], "backfill");
        ws
    })
    .await
    .expect("client");

    emit(&store, 3).await;
    let ws_back = tokio::task::spawn_blocking(move || {
        assert_eq!(read_seqs(&mut ws, 3), vec![6, 7, 8], "live");
        ws.close(None).ok();
        ws
    })
    .await
    .expect("client");
    drop(ws_back);

    // Missed while disconnected.
    emit(&store, 4).await;
    let seqs = tokio::task::spawn_blocking(move || read_seqs(&mut connect(port, 8), 4))
        .await
        .expect("client");
    assert_eq!(
        seqs,
        vec![9, 10, 11, 12],
        "exactly the missed ones, no duplicates"
    );

    // Without a token the upgrade is refused.
    let refused = tokio::task::spawn_blocking(move || {
        tungstenite::connect(format!("ws://127.0.0.1:{port}/v1/stream")).is_err()
    })
    .await
    .expect("client");
    assert!(refused);
}
