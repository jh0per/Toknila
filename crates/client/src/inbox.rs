//! The inbox route: what waits on a person.

use crate::{Client, ClientResult, NO_BODY, enc};

impl Client {
    /// `GET /projects/{project}/inbox`: what waits on a person, newest first.
    pub fn inbox(&self, project: &str) -> ClientResult<Vec<toknila_core::inbox::InboxItem>> {
        self.call("GET", &format!("/projects/{}/inbox", enc(project)), NO_BODY)
    }
}
