//! The runner routes: a ticket's terminal output and typing into it. The raw bytes are not JSON,
//! so these calls bypass the typed-JSON helper; the frame format of the live stream is in
//! `toknila_api_types::runner`.

use std::io::Read;

use toknila_api_types::runner::{HEADER_END, HEADER_OFFSET, HEADER_RUN};
use toknila_api_types::{
    InstructionClaim, NewAnswer, PermissionAsk, PermissionReply, TerminalSize,
};
use toknila_core::{AgentRole, RunId};

use crate::{Client, ClientError, ClientResult, NO_BODY, enc, transport_error};

/// The tail of a ticket's newest run, as the daemon keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrollback {
    pub run: RunId,
    /// Offset of the first byte in the run's output.
    pub offset: u64,
    /// Offset after the last byte; the stream continues from here.
    pub end: u64,
    /// The raw terminal output, ANSI escapes kept.
    pub bytes: Vec<u8>,
}

impl Client {
    /// `GET /tickets/{ticket}/runner/scrollback`: the output of the ticket's runner of `role`;
    /// the reviewer's is read-only.
    pub fn runner_scrollback_of(&self, ticket: &str, role: AgentRole) -> ClientResult<Scrollback> {
        let url = format!(
            "{}/v1/tickets/{}/runner/scrollback{}",
            self.base,
            enc(ticket),
            role_query(role, '?')
        );
        self.scrollback_at(&url)
    }

    /// Reads a scrollback answer (raw bytes and the three headers) from `url`.
    pub(crate) fn scrollback_at(&self, url: &str) -> ClientResult<Scrollback> {
        let resp = self
            .agent
            .get(url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(transport_error)?;
        let header = |name: &str| {
            resp.header(name)
                .map(str::to_owned)
                .ok_or_else(|| ClientError::Decode(format!("answer lacks the {name} header")))
        };
        let number = |name: &str| {
            header(name)?
                .parse::<u64>()
                .map_err(|e| ClientError::Decode(format!("{name}: {e}")))
        };
        let run = header(HEADER_RUN)?
            .parse()
            .map_err(|e| ClientError::Decode(format!("{HEADER_RUN}: {e}")))?;
        let (offset, end) = (number(HEADER_OFFSET)?, number(HEADER_END)?);
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| ClientError::Decode(e.to_string()))?;
        Ok(Scrollback {
            run,
            offset,
            end,
            bytes,
        })
    }

    /// `POST /tickets/{ticket}/runner/input`: types `bytes` into the worker's terminal.
    pub fn runner_input(&self, ticket: &str, bytes: &[u8]) -> ClientResult<()> {
        self.runner_input_of(ticket, AgentRole::Worker, bytes)
    }

    /// Types `bytes` into the terminal of the ticket's runner of `role`.
    pub fn runner_input_of(&self, ticket: &str, role: AgentRole, bytes: &[u8]) -> ClientResult<()> {
        let url = format!(
            "{}/v1/tickets/{}/runner/input{}",
            self.base,
            enc(ticket),
            role_query(role, '?')
        );
        self.agent
            .post(&url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/octet-stream")
            .send_bytes(bytes)
            .map(|_| ())
            .map_err(transport_error)
    }

    /// The WebSocket address of a ticket's live terminal output, continuing after `cursor` (a run
    /// and the offset after the last byte of it that the caller has).
    pub fn runner_stream_url(&self, ticket: &str, cursor: Option<(RunId, u64)>) -> String {
        self.runner_stream_url_of(ticket, AgentRole::Worker, cursor)
    }

    /// The same for the ticket's runner of `role`.
    pub fn runner_stream_url_of(
        &self,
        ticket: &str,
        role: AgentRole,
        cursor: Option<(RunId, u64)>,
    ) -> String {
        let ws = self.base.replacen("http://", "ws://", 1);
        let from = cursor.map_or_else(String::new, |(run, at)| format!("&run={run}&from={at}"));
        format!(
            "{ws}/v1/tickets/{}/runner/stream?token={}{from}{}",
            enc(ticket),
            self.token,
            role_query(role, '&')
        )
    }
}

/// The `role` query parameter; the worker is the default and is not written out.
fn role_query(role: AgentRole, sep: char) -> String {
    match role {
        AgentRole::Reviewer => format!("{sep}role=reviewer"),
        _ => String::new(),
    }
}

impl Client {
    /// `POST /tickets/{ticket}/instructions/claim`: the instructions a person left for the
    /// ticket's agent, handed over once.
    pub fn claim_instructions(&self, ticket: &str) -> ClientResult<InstructionClaim> {
        self.call(
            "POST",
            &format!("/tickets/{}/instructions/claim", enc(ticket)),
            NO_BODY,
        )
    }

    /// `GET /projects/{project}/inbox`: what waits on a person, newest first.
    pub fn inbox(&self, project: &str) -> ClientResult<Vec<toknila_core::inbox::InboxItem>> {
        self.call("GET", &format!("/projects/{}/inbox", enc(project)), NO_BODY)
    }

    /// `POST /tickets/{ticket}/runner/answer`: a person's answer to what the agent asks.
    pub fn answer(&self, ticket: &str, a: &NewAnswer) -> ClientResult<()> {
        self.call(
            "POST",
            &format!("/tickets/{}/runner/answer", enc(ticket)),
            Some(a),
        )
    }

    /// `POST /tickets/{ticket}/permission`: the agent's permission hook asks and waits (up to
    /// `wait`) for the person's decision.
    pub fn permission(
        &self,
        ticket: &str,
        ask: &PermissionAsk,
        wait: std::time::Duration,
    ) -> ClientResult<PermissionReply> {
        let url = format!("{}/v1/tickets/{}/permission", self.base, enc(ticket));
        let json = serde_json::to_value(ask).map_err(|e| ClientError::Decode(e.to_string()))?;
        let text = self
            .agent
            .post(&url)
            .timeout(wait)
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_json(json)
            .map_err(transport_error)?
            .into_string()
            .map_err(|e| ClientError::Decode(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))
    }

    /// `POST /tickets/{ticket}/runner/size`: the size of the panel showing the terminal.
    pub fn runner_size(&self, ticket: &str, size: &TerminalSize) -> ClientResult<()> {
        self.call(
            "POST",
            &format!("/tickets/{}/runner/size", enc(ticket)),
            Some(size),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_address_carries_the_ticket_the_token_and_where_to_continue() {
        let c = Client::new("http://127.0.0.1:4100", "t0k");
        assert_eq!(
            c.runner_stream_url("DEMO-1", None),
            "ws://127.0.0.1:4100/v1/tickets/DEMO-1/runner/stream?token=t0k"
        );
        let run: RunId = "01JBQ7ZJ0000000000000000R1".parse().expect("id");
        assert_eq!(
            c.runner_stream_url("DEMO-1", Some((run, 42))),
            "ws://127.0.0.1:4100/v1/tickets/DEMO-1/runner/stream?token=t0k&run=01JBQ7ZJ0000000000000000R1&from=42"
        );
        assert_eq!(
            c.runner_stream_url_of("DEMO-1", AgentRole::Reviewer, None),
            "ws://127.0.0.1:4100/v1/tickets/DEMO-1/runner/stream?token=t0k&role=reviewer"
        );
    }
}
