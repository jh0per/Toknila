//! Typed blocking client of the daemon's `/v1` API: one method per route, typed bodies in and
//! out, and the daemon's error body as a typed error.

use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use toknila_api_types::ErrorBody;

mod agents;
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
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        Self {
            base: base.into(),
            token: token.into(),
            agent,
        }
    }

    /// Whether a daemon answers `/healthz` at `base`.
    pub fn healthy(base: &str) -> bool {
        ureq::get(&format!("{base}/healthz"))
            .timeout(Duration::from_secs(2))
            .call()
            .is_ok()
    }

    /// The WebSocket address of the live journal, starting after `since`.
    pub fn stream_url(&self, since: i64) -> String {
        let ws = self.base.replacen("http://", "ws://", 1);
        format!("{ws}/v1/stream?since={since}&token={}", self.token)
    }

    fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> ClientResult<T> {
        let req = self
            .agent
            .request(method, &format!("{}/v1{path}", self.base))
            .set("Authorization", &format!("Bearer {}", self.token));
        let resp = match body {
            Some(b) => {
                let json =
                    serde_json::to_value(b).map_err(|e| ClientError::Decode(e.to_string()))?;
                req.send_json(json)
            }
            None => req.call(),
        };
        match resp {
            Ok(r) => {
                let text = r
                    .into_string()
                    .map_err(|e| ClientError::Decode(e.to_string()))?;
                // An empty answer (202 Accepted) reads as JSON null.
                let text = if text.is_empty() { "null" } else { &text };
                serde_json::from_str(text).map_err(|e| ClientError::Decode(e.to_string()))
            }
            Err(e) => Err(transport_error(e)),
        }
    }
}

/// A failed `ureq` call as a [`ClientError`]: the daemon's error body, or the transport failure.
fn transport_error(e: ureq::Error) -> ClientError {
    match e {
        ureq::Error::Status(status, r) => {
            let body = r.into_json().unwrap_or_else(|_| ErrorBody {
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

    #[test]
    fn stream_url_is_websocket_with_token() {
        let c = Client::new("http://127.0.0.1:4100", "t0k");
        assert_eq!(
            c.stream_url(7),
            "ws://127.0.0.1:4100/v1/stream?since=7&token=t0k"
        );
    }
}
