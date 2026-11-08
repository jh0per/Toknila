//! The event routes: the journal, and what a ticket's agent reports.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{AgentEvent, EventRecord, InstructionClaim, NewActivity, TicketDetail};

impl Client {
    /// `GET /events`: journal entries after `since`, oldest first.
    pub fn events(&self, since: i64, limit: usize) -> ClientResult<Vec<EventRecord>> {
        self.call(
            "GET",
            &format!("/events?since={since}&limit={limit}"),
            NO_BODY,
        )
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

    /// `POST /tickets/{ticket}/instructions/claim`: the instructions a person left for the
    /// ticket's agent, handed over once.
    pub fn claim_instructions(&self, ticket: &str) -> ClientResult<InstructionClaim> {
        self.call(
            "POST",
            &format!("/tickets/{}/instructions/claim", enc(ticket)),
            NO_BODY,
        )
    }
}
