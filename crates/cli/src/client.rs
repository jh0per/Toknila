//! Blocking HTTP client of the daemon API. Starts the daemon if it is not running.

use std::time::Duration;

use anyhow::{Result, anyhow};
use serde_json::Value;
use toknila_daemon::paths::Paths;

use crate::daemon_cmd;

/// Authenticated connection to the local daemon.
#[derive(Debug)]
pub struct Client {
    base: String,
    token: String,
}

impl Client {
    /// Connects to the daemon of this home, starting it first if needed.
    pub fn connect(paths: &Paths) -> Result<Client> {
        let info = daemon_cmd::start(paths)?;
        Ok(Client {
            base: info.base_url(),
            token: info.token,
        })
    }

    /// Calls `/v1<path>`; a non-2xx answer becomes an error carrying the daemon's message.
    pub fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        let req = ureq::request(method, &format!("{}/v1{path}", self.base))
            .set("Authorization", &format!("Bearer {}", self.token))
            .timeout(Duration::from_secs(30));
        let resp = match body {
            Some(b) => req.send_json(b.clone()),
            None => req.call(),
        };
        match resp {
            Ok(r) => {
                let text = r.into_string()?;
                if text.is_empty() {
                    Ok(Value::Null)
                } else {
                    Ok(serde_json::from_str(&text)?)
                }
            }
            Err(ureq::Error::Status(code, r)) => {
                let body: Value = r.into_json().unwrap_or(Value::Null);
                let msg = body["error"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("HTTP {code}"));
                Err(anyhow!(msg))
            }
            Err(e) => Err(anyhow!("cannot reach the daemon: {e}")),
        }
    }

    pub fn get(&self, path: &str) -> Result<Value> {
        self.call("GET", path, None)
    }

    pub fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.call("POST", path, Some(&body))
    }

    pub fn patch(&self, path: &str, body: Value) -> Result<Value> {
        self.call("PATCH", path, Some(&body))
    }

    pub fn delete(&self, path: &str) -> Result<Value> {
        self.call("DELETE", path, None)
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
