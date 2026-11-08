//! The plan routes: the plans of a repository, and importing one by path.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{ImportPath, ImportPathResult, PlanScan};

impl Client {
    /// `POST /projects/{project}/import-path`: the daemon reads `path` (absolute) and imports it.
    pub fn import_path(&self, project: &str, path: &str) -> ClientResult<ImportPathResult> {
        let body = ImportPath {
            path: path.to_owned(),
        };
        let route = format!("/projects/{}/import-path", enc(project));
        self.call("POST", &route, Some(&body))
    }

    /// `GET /projects/{project}/plans`: the plans in the project's repository.
    pub fn plans(&self, project: &str) -> ClientResult<PlanScan> {
        self.call("GET", &format!("/projects/{}/plans", enc(project)), NO_BODY)
    }
}
