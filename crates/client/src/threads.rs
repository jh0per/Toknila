//! The thread routes of a card: open a thread, post to it, approve or ask for changes.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{NewMessage, NewThread, NewVersion, ThreadDetail, TicketThreads};

impl Client {
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
}

fn thread_path(ticket: &str, thread: &str, tail: &str) -> String {
    format!("/tickets/{}/threads/{}/{tail}", enc(ticket), enc(thread))
}
