//! Typed blocking client of the daemon's `/v1` API: one method per route, typed bodies in and
//! out, and the daemon's error body as a typed error.

use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use toknila_api_types::{
    ActionInput, AgentEvent, ChainDetail, ChainFile, ChainItem, EdgeInput, ErrorBody, EventRecord,
    NewActivity, NewChain, NewComment, NewMessage, NewProject, NewRelease, NewSubtask, NewThread,
    NewTicket, NewVersion, NewWildcard, PatchSubtask, PatchTicket, ProfileList, ReleaseEvent,
    ReviewSettings, ThreadDetail, TicketDetail, TicketItem, TicketQuery, TicketThreads,
};
use toknila_core::Project;

mod runner;

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

    // ---- projects ----

    /// `GET /projects`.
    pub fn projects(&self) -> ClientResult<Vec<Project>> {
        self.call("GET", "/projects", NO_BODY)
    }

    /// `POST /projects`.
    pub fn create_project(&self, p: &NewProject) -> ClientResult<Project> {
        self.call("POST", "/projects", Some(p))
    }

    // ---- profiles ----

    /// `GET /profiles`.
    pub fn profiles(&self) -> ClientResult<ProfileList> {
        self.call("GET", "/profiles", NO_BODY)
    }

    // ---- chains ----

    /// `POST /projects/{project}/chains`.
    pub fn create_chain(&self, project: &str, c: &NewChain) -> ClientResult<ChainDetail> {
        self.call(
            "POST",
            &format!("/projects/{}/chains", enc(project)),
            Some(c),
        )
    }

    /// `POST /projects/{project}/import`.
    pub fn import_chain(&self, project: &str, toml: String) -> ClientResult<ChainDetail> {
        let path = format!("/projects/{}/import", enc(project));
        self.call("POST", &path, Some(&ChainFile { toml }))
    }

    /// `GET /chains/{chain}/export`.
    pub fn export_chain(&self, chain: &str) -> ClientResult<String> {
        let path = format!("/chains/{}/export", enc(chain));
        Ok(self.call::<ChainFile>("GET", &path, NO_BODY)?.toml)
    }

    /// `GET /chains`.
    pub fn chains(&self) -> ClientResult<Vec<ChainItem>> {
        self.call("GET", "/chains", NO_BODY)
    }

    /// `GET /chains/{chain}`.
    pub fn chain(&self, chain: &str) -> ClientResult<ChainDetail> {
        self.call("GET", &format!("/chains/{}", enc(chain)), NO_BODY)
    }

    /// `POST /chains/{chain}/edges`.
    /// `PATCH /chains/{chain}`: how the chain's tickets are reviewed.
    pub fn patch_chain(&self, chain: &str, s: &ReviewSettings) -> ClientResult<ChainDetail> {
        self.call("PATCH", &format!("/chains/{}", enc(chain)), Some(s))
    }

    /// `PATCH /projects/{project}`: how the project's tickets are reviewed by default.
    pub fn patch_project(&self, project: &str, s: &ReviewSettings) -> ClientResult<Project> {
        self.call("PATCH", &format!("/projects/{}", enc(project)), Some(s))
    }

    pub fn add_edge(&self, chain: &str, e: &EdgeInput) -> ClientResult<ChainDetail> {
        self.call("POST", &format!("/chains/{}/edges", enc(chain)), Some(e))
    }

    /// `DELETE /chains/{chain}/edges`.
    pub fn remove_edge(&self, chain: &str, from: &str, to: &str) -> ClientResult<ChainDetail> {
        let path = format!(
            "/chains/{}/edges?from={}&to={}",
            enc(chain),
            enc(from),
            enc(to)
        );
        self.call("DELETE", &path, NO_BODY)
    }

    // ---- releases ----

    /// `POST /chains/{chain}/releases`.
    pub fn create_release(&self, chain: &str, r: &NewRelease) -> ClientResult<ChainDetail> {
        self.call("POST", &format!("/chains/{}/releases", enc(chain)), Some(r))
    }

    /// `POST /releases/{release}/actions/{action}`: start or complete.
    pub fn release_action(&self, release: &str, action: &str) -> ClientResult<ChainDetail> {
        let path = format!("/releases/{}/actions/{action}", enc(release));
        self.call("POST", &path, Some(&serde_json::json!({})))
    }

    /// `POST /releases/{release}/events`: what a release agent reports.
    pub fn release_event(&self, release: &str, e: &ReleaseEvent) -> ClientResult<ChainDetail> {
        self.call(
            "POST",
            &format!("/releases/{}/events", enc(release)),
            Some(e),
        )
    }

    // ---- tickets ----

    /// `POST /chains/{chain}/tickets`.
    pub fn create_ticket(&self, chain: &str, t: &NewTicket) -> ClientResult<TicketDetail> {
        self.call("POST", &format!("/chains/{}/tickets", enc(chain)), Some(t))
    }

    /// `POST /chains/{chain}/wildcards`: a person proposes a wildcard card. Approve or reject it
    /// with [`Self::ticket_action`] (`approve`, `reject`).
    pub fn propose_wildcard(&self, chain: &str, w: &NewWildcard) -> ClientResult<TicketDetail> {
        let path = format!("/chains/{}/wildcards", enc(chain));
        self.call("POST", &path, Some(w))
    }

    /// `GET /tickets`.
    pub fn tickets(&self, q: &TicketQuery) -> ClientResult<Vec<TicketItem>> {
        let mut path = format!("/tickets?needs_human={}", q.needs_human);
        if let Some(c) = &q.chain {
            path.push_str(&format!("&chain={}", enc(c)));
        }
        self.call("GET", &path, NO_BODY)
    }

    /// `GET /tickets/{ticket}`.
    pub fn ticket(&self, ticket: &str) -> ClientResult<TicketDetail> {
        self.call("GET", &format!("/tickets/{}", enc(ticket)), NO_BODY)
    }

    /// `PATCH /tickets/{ticket}`.
    pub fn patch_ticket(&self, ticket: &str, p: &PatchTicket) -> ClientResult<TicketDetail> {
        self.call("PATCH", &format!("/tickets/{}", enc(ticket)), Some(p))
    }

    /// `POST /tickets/{ticket}/actions/{action}`.
    pub fn ticket_action(
        &self,
        ticket: &str,
        action: &str,
        text: Option<&str>,
    ) -> ClientResult<TicketDetail> {
        let path = format!("/tickets/{}/actions/{action}", enc(ticket));
        let body = ActionInput {
            text: text.map(str::to_owned),
        };
        self.call("POST", &path, Some(&body))
    }

    /// `POST /tickets/{ticket}/comments`.
    pub fn add_comment(&self, ticket: &str, c: &NewComment) -> ClientResult<TicketDetail> {
        self.call(
            "POST",
            &format!("/tickets/{}/comments", enc(ticket)),
            Some(c),
        )
    }

    // ---- threads on a card ----

    /// `GET /tickets/{ticket}/threads`.
    pub fn threads(&self, ticket: &str) -> ClientResult<TicketThreads> {
        self.call("GET", &format!("/tickets/{}/threads", enc(ticket)), NO_BODY)
    }

    /// `POST /tickets/{ticket}/threads`.
    pub fn open_thread(&self, ticket: &str, t: &NewThread) -> ClientResult<ThreadDetail> {
        let path = format!("/tickets/{}/threads", enc(ticket));
        self.call("POST", &path, Some(t))
    }

    /// `POST /tickets/{ticket}/threads/{thread}/messages`.
    pub fn post_thread_message(
        &self,
        ticket: &str,
        thread: &str,
        m: &NewMessage,
    ) -> ClientResult<ThreadDetail> {
        self.call("POST", &thread_path(ticket, thread, "messages"), Some(m))
    }

    /// `POST /tickets/{ticket}/threads/{thread}/versions`.
    pub fn post_thread_version(
        &self,
        ticket: &str,
        thread: &str,
        v: &NewVersion,
    ) -> ClientResult<ThreadDetail> {
        self.call("POST", &thread_path(ticket, thread, "versions"), Some(v))
    }

    /// `POST /tickets/{ticket}/threads/{thread}/approve`.
    pub fn approve_thread(&self, ticket: &str, thread: &str) -> ClientResult<ThreadDetail> {
        self.call("POST", &thread_path(ticket, thread, "approve"), NO_BODY)
    }

    /// `POST /tickets/{ticket}/threads/{thread}/changes`.
    pub fn request_thread_changes(
        &self,
        ticket: &str,
        thread: &str,
        m: &NewMessage,
    ) -> ClientResult<ThreadDetail> {
        self.call("POST", &thread_path(ticket, thread, "changes"), Some(m))
    }

    /// `POST /tickets/{ticket}/subtasks`.
    pub fn add_subtask(&self, ticket: &str, text: &str) -> ClientResult<TicketDetail> {
        let body = NewSubtask {
            text: text.to_owned(),
        };
        self.call(
            "POST",
            &format!("/tickets/{}/subtasks", enc(ticket)),
            Some(&body),
        )
    }

    /// `PATCH /tickets/{ticket}/subtasks/{position}`.
    pub fn patch_subtask(
        &self,
        ticket: &str,
        position: u32,
        p: &PatchSubtask,
    ) -> ClientResult<TicketDetail> {
        let path = format!("/tickets/{}/subtasks/{position}", enc(ticket));
        self.call("PATCH", &path, Some(p))
    }

    /// `POST /tickets/{ticket}/events`: what a ticket's agent reports. The answer is the ticket,
    /// or the new card for `propose`.
    pub fn agent_event(&self, ticket: &str, e: &AgentEvent) -> ClientResult<TicketDetail> {
        self.call("POST", &format!("/tickets/{}/events", enc(ticket)), Some(e))
    }

    /// `POST /tickets/{ticket}/activity`: a sign of life from the ticket's agent.
    pub fn activity(&self, ticket: &str, a: &NewActivity) -> ClientResult<()> {
        self.call(
            "POST",
            &format!("/tickets/{}/activity", enc(ticket)),
            Some(a),
        )
    }

    // ---- journal and daemon ----

    /// `GET /events`: journal entries after `since`, oldest first.
    pub fn events(&self, since: i64, limit: usize) -> ClientResult<Vec<EventRecord>> {
        self.call(
            "GET",
            &format!("/events?since={since}&limit={limit}"),
            NO_BODY,
        )
    }

    /// `POST /shutdown`: asks the daemon to stop.
    pub fn shutdown(&self) -> ClientResult<()> {
        self.call::<serde_json::Value>("POST", "/shutdown", NO_BODY)
            .map(|_| ())
    }
}

/// A failed `ureq` call as a [`ClientError`]: the daemon's error body, or the transport failure.
fn transport_error(e: ureq::Error) -> ClientError {
    match e {
        ureq::Error::Status(status, r) => {
            let body = r.into_json().unwrap_or_else(|_| ErrorBody {
                error: format!("HTTP {status}"),
                code: String::new(),
            });
            ClientError::Api { status, body }
        }
        e => ClientError::Transport(e.to_string()),
    }
}

fn thread_path(ticket: &str, thread: &str, tail: &str) -> String {
    format!("/tickets/{}/threads/{}/{tail}", enc(ticket), enc(thread))
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

    #[test]
    fn stream_url_is_websocket_with_token() {
        let c = Client::new("http://127.0.0.1:4100", "t0k");
        assert_eq!(
            c.stream_url(7),
            "ws://127.0.0.1:4100/v1/stream?since=7&token=t0k"
        );
    }
}
