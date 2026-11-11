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

impl Client {
    /// `GET /chains/{chain}/release`: what the Release dialog offers.
    pub fn release_offer(&self, chain: &str) -> ClientResult<toknila_api_types::ReleaseOffer> {
        self.call(
            "GET",
            &format!("/chains/{}/release", enc(chain)),
            crate::NO_BODY,
        )
    }

    /// `POST /chains/{chain}/release`: the click on Release; the run goes on in the daemon.
    pub fn start_release(
        &self,
        chain: &str,
        version: &str,
        resume: bool,
    ) -> ClientResult<ChainDetail> {
        let body = toknila_api_types::StartRelease {
            version: version.to_owned(),
            resume,
        };
        self.call(
            "POST",
            &format!("/chains/{}/release", enc(chain)),
            Some(&body),
        )
    }

    /// `GET /releases/{release}/run`: the steps of the last release run.
    pub fn release_run(&self, release: &str) -> ClientResult<toknila_api_types::ReleaseRun> {
        self.call(
            "GET",
            &format!("/releases/{}/run", enc(release)),
            crate::NO_BODY,
        )
    }
}
