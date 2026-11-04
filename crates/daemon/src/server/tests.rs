use std::net::Ipv4Addr;

use super::*;
use crate::info::{DaemonInfo, new_token};

async fn spawn_test_server() -> (
    tempfile::TempDir,
    u16,
    String,
    Arc<Notify>,
    tokio::task::JoinHandle<()>,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("t.db")).expect("store");
    let shutdown = Arc::new(Notify::new());
    let token = "secret-token".to_owned();
    let state = AppState {
        store,
        token: token.clone().into(),
        shutdown: shutdown.clone(),
    };
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let stop = shutdown.clone();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async move { stop.notified().await })
            .await
            .expect("serve");
    });
    (dir, port, token, shutdown, handle)
}

fn status_of(r: Result<ureq::Response, ureq::Error>) -> u16 {
    match r {
        Ok(resp) => resp.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(e) => panic!("transport error: {e}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn healthz_is_open_and_v1_needs_the_token() {
    let (_dir, port, token, _shutdown, handle) = spawn_test_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let (health, no_token, wrong, right) = tokio::task::spawn_blocking(move || {
        let health = status_of(ureq::get(&format!("{base}/healthz")).call());
        let no_token = status_of(ureq::post(&format!("{base}/v1/shutdown")).call());
        let wrong = status_of(
            ureq::post(&format!("{base}/v1/shutdown"))
                .set("Authorization", "Bearer nope")
                .call(),
        );
        let right = status_of(
            ureq::post(&format!("{base}/v1/shutdown"))
                .set("Authorization", &format!("Bearer {token}"))
                .call(),
        );
        (health, no_token, wrong, right)
    })
    .await
    .expect("client");
    assert_eq!((health, no_token, wrong, right), (200, 401, 401, 202));
    // The accepted shutdown stops the server.
    tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("stopped")
        .expect("join");
}

#[test]
fn token_compare() {
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"abcd"));
}

#[test]
fn daemon_info_roundtrips_through_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("daemon.json");
    assert_eq!(DaemonInfo::read(&path).expect("read"), None);
    let info = DaemonInfo {
        pid: 7,
        port: 4242,
        token: new_token().expect("token"),
        version: "0.0.1".into(),
        started_at: 1,
    };
    assert_eq!(info.token.len(), 64);
    info.write(&path).expect("write");
    assert_eq!(DaemonInfo::read(&path).expect("read"), Some(info));
}
