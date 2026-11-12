//! The ticket routes: create and read a card, change it, act on it, comment, subtasks.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{
    ActionInput, DesignMoveInput, MoveToChain, NewComment, NewSubtask, NewTicket, NewWildcard,
    PatchSubtask, PatchTicket, TicketDetail, TicketItem, TicketQuery,
};

impl Client {
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

    /// `POST /tickets/{ticket}/designs/{index}/move`.
    pub fn move_design(
        &self,
        ticket: &str,
        index: usize,
        m: &DesignMoveInput,
    ) -> ClientResult<TicketDetail> {
        let path = format!("/tickets/{}/designs/{index}/move", enc(ticket));
        self.call("POST", &path, Some(m))
    }

    /// `POST /tickets/{ticket}/move-to-chain`.
    pub fn move_to_chain(&self, ticket: &str, m: &MoveToChain) -> ClientResult<TicketDetail> {
        let path = format!("/tickets/{}/move-to-chain", enc(ticket));
        self.call("POST", &path, Some(m))
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
}
