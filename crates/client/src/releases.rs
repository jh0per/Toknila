//! The release routes: add a release, start or complete it, and what its agent reports.

use crate::{Client, ClientResult, enc};
use toknila_api_types::{ChainDetail, NewRelease, ReleaseEvent};

impl Client {
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
}
