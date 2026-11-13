//! Typed blocking client of the daemon's `/v1` API: one method per route, typed bodies in and
//! out, and the daemon's error body as a typed error.

use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use toknila_api_types::ErrorBody;

mod agents;
mod body;
mod chains;
mod events;
mod inbox;
mod planner;
mod plans;
mod projects;
mod questions;
mod releases;
mod runner;
mod system;
mod threads;
mod tickets;
mod usage;

pub use runner::Scrollback;

/// A failed call.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The daemon answered with an error status; its message is shown as is.
    #[error("{}", .body.error)]
    Api { status: u16, body: ErrorBody },
    /// The daemon could not be reached.
    #[error("cannot reach the daemon: {0}")]
    Transport(String),
    /// The daemon's answer did not have the expected shape.
    #[error("unexpected answer from the daemon: {0}")]
    Decode(String),
}

/// Result of a call.
pub type ClientResult<T> = Result<T, ClientError>;

/// Authenticated connection to a daemon.
#[derive(Debug)]
pub struct Client {
    base: String,
    token: String,
    agent: ureq::Agent,
}

/// A body-less request.
const NO_BODY: Option<&()> = None;

impl Client {
    /// A client of the daemon at `base` (`http://127.0.0.1:<port>`) with its bearer token.
    pub fn new(base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            token: token.into(),
            agent: agent(Duration::from_secs(30)),
        }
    }

    /// Whether a daemon answers `/healthz` at `base`.
    pub fn healthy(base: &str) -> bool {
        agent(Duration::from_secs(2))
            .get(&format!("{base}/healthz"))
            .call()
            .is_ok()
    }

    /// The WebSocket address of the live journal, starting after `since`.
    pub fn stream_url(&self, since: i64) -> String {
        let ws = self.base.replacen("http://", "ws://", 1);
        format!("{ws}/v1/stream?since={since}&token={}", self.token)
    }

    /// One call. A call that lost its connection is repeated once on a new one only when the
    /// method is idempotent (`GET`, `PUT`, `DELETE`); a `POST` is never sent twice, since the
    /// daemon may have acted on it before the connection went.
    fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> ClientResult<T> {
        self.call_as(
            method,
            path,
            body,
            matches!(method, "GET" | "PUT" | "DELETE"),
        )
    }

    /// Like [`Client::call`] for a route whose repeat is harmless whatever its method (the
    /// daemon documents it: a second stop of a stopped planner is fine).
    fn call_idempotent<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> ClientResult<T> {
        self.call_as(method, path, body, true)
    }

    fn call_as<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
        repeatable: bool,
    ) -> ClientResult<T> {
        let json = body
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| ClientError::Decode(e.to_string()))?;
        let mut tries = if repeatable { 2 } else { 1 };
        let text = loop {
            tries -= 1;
            match self.once(method, path, json.as_ref()) {
                Ok(text) => break text,
                Err((_, true)) if tries > 0 => {}
                Err((e, _)) => return Err(e),
            }
        };
        // An empty answer (202 Accepted) reads as JSON null.
        let text = if text.is_empty() { "null" } else { &text };
        serde_json::from_str(text).map_err(|e| ClientError::Decode(e.to_string()))
    }

    /// One try: the answer's text, or the error and whether the connection was lost.
    fn once(
        &self,
        method: &str,
        path: &str,
        json: Option<&serde_json::Value>,
    ) -> Result<String, (ClientError, bool)> {
        let req = self
            .agent
            .request(method, &format!("{}/v1{path}", self.base))
            .set("Authorization", &format!("Bearer {}", self.token));
        let resp = match json {
            Some(j) => req.send_json(j.clone()),
            None => req.call(),
        };
        let what = format!("{method} {path}");
        match resp {
            Ok(r) => body::text(r).map_err(|e| {
                let lost = body::lost(&e);
                (operation_failed(&what, "read the answer's body", &e), lost)
            }),
            Err(ureq::Error::Transport(t)) => {
                use ureq::ErrorKind::{ConnectionFailed, Dns};
                let op = match t.kind() {
                    Dns => "resolve the daemon's address",
                    ConnectionFailed => "connect",
                    _ => "send the request or read the answer's headers",
                };
                let lost = body::lost(&t);
                Err((operation_failed(&what, op, &t), lost))
            }
            Err(e) => Err((transport_error(e), false)),
        }
    }
}

/// An OS-level failure of one step of a call, with the step and the call named: an `os error`
/// alone says nothing about which socket operation it came from.
fn operation_failed(what: &str, operation: &str, e: &dyn std::fmt::Display) -> ClientError {
    ClientError::Transport(format!("{what}: could not {operation}: {e}"))
}

/// An HTTP agent whose limits are per operation, each set once on the socket. Never use ureq's
/// overall `timeout`: it re-arms the socket timeouts before every read, and macOS refuses that
/// with `EINVAL` once the daemon has answered and closed, which surfaced as `Error encountered
/// in a header: Invalid argument (os error 22)` on a slow, loaded client.
pub(crate) fn agent(limit: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(limit)
        .timeout_read(limit)
        .timeout_write(limit)
        .build()
}

/// A failed `ureq` call as a [`ClientError`]: the daemon's error body, or the transport failure.
fn transport_error(e: ureq::Error) -> ClientError {
    match e {
        ureq::Error::Status(status, r) => {
            let body = body::text(r)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| ErrorBody {
                    error: format!("HTTP {status}"),
                    code: String::new(),
                    field: None,
                    used_by: None,
                });
            ClientError::Api { status, body }
        }
        e => ClientError::Transport(e.to_string()),
    }
}

/// Percent-encodes a path segment or query value.
pub fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_everything_but_unreserved() {
        assert_eq!(enc("DEMO-1"), "DEMO-1");
        assert_eq!(enc("a b/c?"), "a%20b%2Fc%3F");
        assert_eq!(enc("0.1.0"), "0.1.0");
    }

    /// A daemon refusing a body answers 400 with its message; the client's error is that message
    /// and nothing else, which is what `tok` prints after `error: `.
    #[test]
    fn a_refused_body_surfaces_the_daemons_message() {
        use std::io::{Read, Write};
        let message = "unknown field `review_modee`, expected one of `max_parallel`, `review_mode`";
        let body = serde_json::json!({ "code": "bad_request", "error": message }).to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let reply = format!(
                "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(reply.as_bytes()).expect("write");
        });
        let err = Client::new(base, "t0k").projects().expect_err("refused");
        server.join().expect("server");
        assert!(
            matches!(err, ClientError::Api { status: 400, .. }),
            "{err:?}"
        );
        assert_eq!(err.to_string(), message);
    }

    /// A daemon that has closed before the headers end must not make the client re-arm socket
    /// timeouts on the dead socket: macOS answers that with `os error 22`.
    #[test]
    fn a_peer_that_closed_mid_headers_is_not_an_invalid_argument() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            // Let the request arrive, never read it: closing with unread data resets the socket.
            std::thread::sleep(Duration::from_millis(200));
            s.write_all(b"HTTP/1.1 200 OK\r\n").expect("write");
            drop(s);
        });
        let err = Client::new(base, "t0k").projects().expect_err("cut");
        server.join().expect("server");
        let text = err.to_string();
        assert!(!text.contains("os error 22"), "{text}");
        assert!(!text.contains("Invalid argument"), "{text}");
    }

    /// A daemon that answers completely and closes at once (a reset, as the unread request is
    /// still in its buffer) must not turn the finished answer into `os error 22`, nor an error
    /// answer into a bare "HTTP 400".
    ///
    /// macOS only: the `os error 22` of the pool handoff is macOS's answer to a reset socket.
    /// Linux and Windows read the answer whole on the first connection and never open the second
    /// one, so the fake server's `join` would wait forever there (it hung CI for the whole job).
    /// On macOS the reset races the handoff too: when it comes late the answer is read whole and
    /// nothing is repeated, so the test asserts what holds either way (a GET never fails with
    /// the reset, a POST is sent once) and never that the reset was seen.
    #[cfg(target_os = "macos")]
    #[test]
    fn an_answer_the_daemon_closed_right_after_is_read_whole() {
        // Accepts up to `connections`: the first is answered and reset (the request stays
        // unread), the next ones after their request was read. A connection that does not come
        // within two seconds ends the server, so a missing or a wrong retry fails an assertion
        // instead of hanging. Returns how many connections it accepted.
        fn serve(connections: usize) -> (String, std::thread::JoinHandle<usize>) {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.set_nonblocking(true).expect("nonblocking");
            let base = format!("http://{}", listener.local_addr().expect("addr"));
            let server = std::thread::spawn(move || {
                let mut accepted = 0;
                for n in 0..connections {
                    let deadline = std::time::Instant::now() + Duration::from_secs(2);
                    let mut s = loop {
                        match listener.accept() {
                            Ok((s, _)) => break s,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                if std::time::Instant::now() > deadline {
                                    return accepted;
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            Err(e) => panic!("accept: {e}"),
                        }
                    };
                    accepted += 1;
                    s.set_nonblocking(false).expect("blocking");
                    if n > 0 {
                        // The whole request (headers and body may arrive apart), so that the
                        // close is a plain FIN and not a reset.
                        s.set_read_timeout(Some(Duration::from_millis(300)))
                            .expect("timeout");
                        while s.read(&mut [0u8; 4096]).is_ok_and(|n| n > 0) {}
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    let reply = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n[]";
                    s.write_all(reply.as_bytes()).expect("write");
                }
                accepted
            });
            (base, server)
        }

        // A GET whose connection was lost is repeated once on a new connection: the answer is
        // the list, never the reset.
        let (base, server) = serve(2);
        let got = Client::new(base, "t0k").projects();
        let accepted = server.join().expect("server");
        assert!(got.expect("the list, repeated if lost").is_empty());
        assert!((1..=2).contains(&accepted), "{accepted} connections");

        // A route that is harmless to repeat is too (its answer here is not a planner state, so
        // it is a decode error, but never `os error 22`).
        let (base, server) = serve(2);
        let got = Client::new(base, "t0k").stop_planner("DEMO");
        server.join().expect("server");
        let text = format!("{got:?}");
        assert!(!text.contains("os error 22"), "{text}");

        // A POST is sent once: its failure (the reset, or the answer that is no planner state)
        // is shown, and no second connection is opened.
        let (base, server) = serve(2);
        let err = Client::new(base, "t0k")
            .start_planner("DEMO", None)
            .expect_err("no planner state in `[]`");
        let accepted = server.join().expect("server");
        assert_eq!(accepted, 1, "a POST is never repeated: {err}");
    }

    /// A fake daemon: each entry of `plan` is what one connection gets, in order: `true` reads
    /// the request and closes without a word, `false` reads it and answers a stopped planner.
    /// A connection that does not come within two seconds ends the server, so a missing retry
    /// fails an assertion instead of hanging. Returns how many connections it accepted.
    fn fake_daemon(plan: Vec<bool>) -> (String, std::thread::JoinHandle<usize>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let server = std::thread::spawn(move || {
            let mut accepted = 0;
            for drop_it in plan {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                let mut s = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() > deadline {
                                return accepted;
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("accept: {e}"),
                    }
                };
                accepted += 1;
                s.set_nonblocking(false).expect("blocking");
                // The whole request, so that the close is a plain FIN and not a reset.
                s.set_read_timeout(Some(Duration::from_millis(300)))
                    .expect("timeout");
                while s.read(&mut [0u8; 4096]).is_ok_and(|n| n > 0) {}
                if drop_it {
                    continue;
                }
                let body = r#"{"running":false,"run":null,"profile":"fake"}"#;
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(reply.as_bytes());
            }
            accepted
        });
        (base, server)
    }

    /// A stop whose connection the daemon closed once is repeated on a new one and answers: the
    /// stop route is idempotent by design.
    #[test]
    fn a_stop_whose_connection_was_dropped_once_is_repeated() {
        let (base, server) = fake_daemon(vec![true, false]);
        let got = Client::new(base, "t0k").stop_planner("DEMO");
        let accepted = server.join().expect("server");
        let state = got.expect("the second connection answers");
        assert_eq!((state.running, state.profile.as_str()), (false, "fake"));
        assert_eq!(accepted, 2);
    }

    /// A start is not idempotent: a connection dropped once fails it, naming the call and the
    /// step, and no second connection is opened.
    #[test]
    fn a_start_whose_connection_was_dropped_is_not_repeated_and_names_the_step() {
        let (base, server) = fake_daemon(vec![true, false]);
        let err = Client::new(base, "t0k")
            .start_planner("DEMO", None)
            .expect_err("dropped");
        let accepted = server.join().expect("server");
        let text = err.to_string();
        assert!(
            text.contains("POST /projects/DEMO/planner/start: could not "),
            "{text}"
        );
        assert_eq!(accepted, 1, "{text}");
    }

    #[test]
    fn stream_url_is_websocket_with_token() {
        let c = Client::new("http://127.0.0.1:4100", "t0k");
        assert_eq!(
            c.stream_url(7),
            "ws://127.0.0.1:4100/v1/stream?since=7&token=t0k"
        );
    }
}
