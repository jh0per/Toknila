//! A call on a connection the daemon closed while it was idle neither fails nor is sent twice.
//! The real daemon sits behind a loopback relay that closes a connection (a FIN, as hyper does)
//! once it has been idle for `IDLE`, and counts the requests it forwards.

mod common;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::{Tok, tempdir};
use toknila_client::{Client, ClientError};

const IDLE: Duration = Duration::from_millis(300);

/// Connections the relay closed because they were idle (this binary has one relay).
static CLOSED_WHILE_IDLE: AtomicUsize = AtomicUsize::new(0);

/// Waits, with a deadline, until the relay has closed `n` idle connections in all.
fn wait_for_closed(n: usize) {
    let until = Instant::now() + Duration::from_secs(30);
    while CLOSED_WHILE_IDLE.load(Ordering::SeqCst) < n {
        assert!(
            Instant::now() < until,
            "the relay never closed idle connection {n}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// Relays one connection to `upstream`; closes both halves after `IDLE` without traffic.
fn relay(client: TcpStream, upstream: TcpStream, posts: Arc<AtomicUsize>) {
    let last = Arc::new(idle_clock::Last::new());
    let done = Arc::new(AtomicBool::new(false));
    let pumps = [
        (client.try_clone(), upstream.try_clone(), true),
        (upstream.try_clone(), client.try_clone(), false),
    ];
    for (from, to, counts) in pumps {
        let (mut from, mut to) = (from.expect("clone"), to.expect("clone"));
        let (last, done, posts) = (last.clone(), done.clone(), posts.clone());
        thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            while let Ok(n) = from.read(&mut buf) {
                if n == 0 {
                    break;
                }
                last.touch();
                if counts && buf[..n].starts_with(b"POST ") {
                    posts.fetch_add(1, Ordering::SeqCst);
                }
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            done.store(true, Ordering::SeqCst);
            let _ = to.shutdown(Shutdown::Both);
        });
    }
    while !done.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(20));
        if last.idle() > IDLE {
            let _ = client.shutdown(Shutdown::Both);
            let _ = upstream.shutdown(Shutdown::Both);
            CLOSED_WHILE_IDLE.fetch_add(1, Ordering::SeqCst);
            break;
        }
    }
}

mod idle_clock {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// When traffic last passed.
    pub struct Last(Mutex<Instant>);

    impl Last {
        pub fn new() -> Self {
            Self(Mutex::new(Instant::now()))
        }
        pub fn touch(&self) {
            *self.0.lock().expect("lock") = Instant::now();
        }
        pub fn idle(&self) -> Duration {
            self.0.lock().expect("lock").elapsed()
        }
    }
}

/// Starts the relay in front of `target` (`host:port`); returns its base URL and the POST count.
fn start_relay(target: String) -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let posts = Arc::new(AtomicUsize::new(0));
    let conns = Arc::new(AtomicUsize::new(0));
    let (counted, accepted) = (posts.clone(), conns.clone());
    // Detached: it ends with the test process.
    thread::spawn(move || {
        for client in listener.incoming().flatten() {
            accepted.fetch_add(1, Ordering::SeqCst);
            let Ok(upstream) = TcpStream::connect(&target) else {
                continue;
            };
            let posts = counted.clone();
            thread::spawn(move || relay(client, upstream, posts));
        }
    });
    (base, posts, conns)
}

#[test]
fn calls_after_the_daemon_closed_an_idle_connection_succeed_and_a_post_is_sent_once() {
    let dir = tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.raw(&["project", "list"]); // starts the daemon
    let info = tok.info();
    let (base, posts, conns) = start_relay(format!("127.0.0.1:{}", info.port));
    let client = Client::new(base, info.token);

    for round in 0..3 {
        client
            .projects()
            .unwrap_or_else(|e| panic!("GET {round}: {e}"));
        wait_for_closed(round * 3 + 1);
        // The daemon answers 404 for an unknown project: the request got through and was read.
        let err = client.stop_planner("NOPE").expect_err("no such project");
        assert!(
            matches!(err, ClientError::Api { status: 404, .. }),
            "POST {round}: {err:?}"
        );
        wait_for_closed(round * 3 + 2);
        client
            .projects()
            .unwrap_or_else(|e| panic!("GET after POST {round}: {e}"));
        wait_for_closed(round * 3 + 3);
    }
    assert_eq!(
        posts.load(Ordering::SeqCst),
        3,
        "each POST reached the daemon once"
    );
    // Control: every call found its connection closed, so each one opened a new connection.
    assert_eq!(
        conns.load(Ordering::SeqCst),
        9,
        "the relay closed every idle connection"
    );
}
