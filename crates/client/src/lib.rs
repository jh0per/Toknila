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
        match resp {
            Ok(r) => {
                body::text(r).map_err(|e| (ClientError::Decode(e.to_string()), body::lost(&e)))
            }
            Err(e) => {
                let lost = matches!(&e, ureq::Error::Transport(t) if body::lost(t));
                Err((transport_error(e), lost))
            }
        }
    }
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
    #[test]
    fn an_answer_the_daemon_closed_right_after_is_read_whole() {
        // The first connection is answered and reset (the request stays unread); the next ones
        // are answered after their request was read. Returns the connections it accepted.
        fn serve(connections: usize) -> (String, std::thread::JoinHandle<()>) {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            let base = format!("http://{}", listener.local_addr().expect("addr"));
            let server = std::thread::spawn(move || {
                for n in 0..connections {
                    let (mut s, _) = listener.accept().expect("accept");
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
            });
            (base, server)
        }

        // A GET whose connection was lost is repeated once on a new connection.
        let (base, server) = serve(2);
        let got = Client::new(base, "t0k").projects();
        server.join().expect("server");
        assert!(got.expect("repeated").is_empty());

        // A route that is harmless to repeat is too (its answer here is not a planner state, so
        // it is a decode error, but it came from the second connection, not from `os error 22`).
        let (base, server) = serve(2);
        let got = Client::new(base, "t0k").stop_planner("DEMO");
        server.join().expect("server");
        let text = format!("{got:?}");
        assert!(!text.contains("os error 22"), "{text}");

        // A POST is sent once: its failure is shown, and no second connection was opened (the
        // server would otherwise wait for it, and `join` would never return).
        let (base, server) = serve(1);
        let err = Client::new(base, "t0k")
            .start_planner("DEMO", None)
            .expect_err("a POST is never repeated");
        server.join().expect("server");
        assert!(err.to_string().contains("os error 22"), "{err}");
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
