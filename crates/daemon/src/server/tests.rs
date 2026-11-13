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
    let token = "secret-token".to_owned();
    let state = AppState::for_test(store, &token, dir.path());
    let shutdown = state.shutdown.clone();
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
    tokio::time::timeout(std::time::Duration::from_secs(60), handle)
        .await
        .expect("stopped")
        .expect("join");
}

/// A stop request whose body (`{}`) arrives in a second segment after its headers, as the client
/// sends it under load, must be answered whole and leave the connection usable. The stop route
/// once never read the body: hyper then answered before the body came, closed the connection and
/// reset the late bytes, which macOS reports to the client as `os error 22` when it sets a socket
/// option on the reset socket. The invariant holds whichever way the segments race: a second
/// request on the same connection is answered.
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_whose_body_comes_late_leaves_the_connection_usable() {
    use std::io::{Read, Write};
    use std::time::Duration;

    // One answer: the headers, then as many body bytes as content-length says. Every read has
    // the socket's deadline.
    fn answer(s: &mut std::net::TcpStream) -> Result<String, String> {
        let mut got = Vec::new();
        let mut one = [0u8; 1];
        while !got.ends_with(b"\r\n\r\n") {
            match s.read(&mut one) {
                Ok(1) => got.push(one[0]),
                Ok(_) => return Err(format!("closed after {:?}", String::from_utf8_lossy(&got))),
                Err(e) => return Err(format!("{e} after {:?}", String::from_utf8_lossy(&got))),
            }
        }
        let text = String::from_utf8_lossy(&got).to_lowercase();
        let n: usize = text
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let mut body = vec![0u8; n];
        s.read_exact(&mut body).map_err(|e| e.to_string())?;
        Ok(format!(
            "{}{}",
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&body)
        ))
    }

    let (_dir, port, token, shutdown, handle) = spawn_test_server().await;
    let outcome = tokio::task::spawn_blocking(move || {
        let base = format!("http://127.0.0.1:{port}");
        let made = ureq::post(&format!("{base}/v1/projects"))
            .set("Authorization", &format!("Bearer {token}"))
            .send_json(serde_json::json!({
                "key": "DEMO", "name": "Demo", "default_profile": "sim", "simulated": true
            }));
        assert!(made.is_ok(), "{made:?}");

        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        s.set_nodelay(true).expect("nodelay");
        let head = format!(
            "POST /v1/projects/DEMO/planner/stop HTTP/1.1\r\nhost: x\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n"
        );
        s.write_all(head.as_bytes()).expect("headers");
        std::thread::sleep(Duration::from_millis(300));
        let late = s.write_all(b"{}");

        let first = answer(&mut s);
        let again = format!(
            "GET /v1/projects/DEMO/planner HTTP/1.1\r\nhost: x\r\nauthorization: Bearer {token}\r\n\r\n"
        );
        let second = s
            .write_all(again.as_bytes())
            .map_err(|e| format!("second request: {e}"))
            .and_then(|()| answer(&mut s));
        (late.map_err(|e| e.to_string()), first, second)
    })
    .await
    .expect("join");
    shutdown.notify_one();
    tokio::time::timeout(Duration::from_secs(60), handle)
        .await
        .expect("stopped")
        .expect("join");

    let (late, first, second) = outcome;
    assert_eq!(late, Ok(()));
    let first = first.expect("the stop is answered");
    assert!(first.starts_with("HTTP/1.1 200"), "{first}");
    let second = second.expect("the same connection answers a second request");
    assert!(second.starts_with("HTTP/1.1 200"), "{second}");
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
        exe: Some("/app/toknila".into()),
    };
    assert_eq!(info.token.len(), 64);
    info.write(&path).expect("write");
    assert_eq!(DaemonInfo::read(&path).expect("read"), Some(info));
}
