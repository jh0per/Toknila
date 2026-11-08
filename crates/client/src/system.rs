//! The system routes: the folder check and stopping the daemon.

use crate::{Client, ClientResult, NO_BODY};
use toknila_api_types::RepoCheckRequest;
use toknila_core::repo::RepoCheck;

impl Client {
    /// `POST /repos/check`: what a folder suggests for a project (key, name, base branch, check
    /// command, merge mode) and what is wrong with it.
    pub fn check_repo(
        &self,
        repo_path: &str,
        work_branch: Option<&str>,
    ) -> ClientResult<RepoCheck> {
        let body = RepoCheckRequest {
            repo_path: repo_path.to_owned(),
            work_branch: work_branch.map(str::to_owned),
        };
        self.call("POST", "/repos/check", Some(&body))
    }

    /// `POST /shutdown`: asks the daemon to stop.
    pub fn shutdown(&self) -> ClientResult<()> {
        self.call::<serde_json::Value>("POST", "/shutdown", NO_BODY)
            .map(|_| ())
    }
}
