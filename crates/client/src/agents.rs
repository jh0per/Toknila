//! The agent routes: what the projects run their cards with, and the profile list.

use crate::{Client, ClientResult, NO_BODY};
use toknila_api_types::{ProfileList, ProjectAgents};

impl Client {
    /// `GET /agents`.
    pub fn agents(&self) -> ClientResult<Vec<ProjectAgents>> {
        self.call("GET", "/agents", NO_BODY)
    }

    /// `GET /profiles`.
    pub fn profiles(&self) -> ClientResult<ProfileList> {
        self.call("GET", "/profiles", NO_BODY)
    }
}
