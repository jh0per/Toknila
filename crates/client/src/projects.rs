//! Projects and their plans: create, change, archive and delete a project, move a card, import
//! by path, and the plans of a repository.

use toknila_api_types::{
    ChainDeletion, ChainDetail, DeleteProject, ImportPath, ImportPathResult, MoveInput, NewProject,
    PatchProject, PlanScan, ProjectDeletion,
};
use toknila_core::Project;

use crate::{Client, ClientResult, NO_BODY, enc};

impl Client {
    /// `GET /projects`.
    pub fn projects(&self) -> ClientResult<Vec<Project>> {
        self.call("GET", "/projects", NO_BODY)
    }

    /// `POST /projects`.
    pub fn create_project(&self, p: &NewProject) -> ClientResult<Project> {
        self.call("POST", "/projects", Some(p))
    }

    /// `PATCH /projects/{project}`.
    pub fn patch_project(&self, project: &str, p: &PatchProject) -> ClientResult<Project> {
        self.call("PATCH", &format!("/projects/{}", enc(project)), Some(p))
    }

    /// `GET /projects/{project}`.
    pub fn project(&self, project: &str) -> ClientResult<Project> {
        self.call("GET", &format!("/projects/{}", enc(project)), NO_BODY)
    }

    /// `POST /projects/{project}/archive`: hidden from the sidebar, data kept.
    pub fn archive_project(&self, project: &str) -> ClientResult<Project> {
        let path = format!("/projects/{}/archive", enc(project));
        self.call("POST", &path, Some(&serde_json::json!({})))
    }

    /// `POST /projects/{project}/unarchive`.
    pub fn unarchive_project(&self, project: &str) -> ClientResult<Project> {
        let path = format!("/projects/{}/unarchive", enc(project));
        self.call("POST", &path, Some(&serde_json::json!({})))
    }

    /// `POST /projects/{project}/delete`: the records go; the answer says what became of the
    /// worktrees, run files and the repository folder.
    pub fn delete_project(
        &self,
        project: &str,
        d: &DeleteProject,
    ) -> ClientResult<ProjectDeletion> {
        self.call(
            "POST",
            &format!("/projects/{}/delete", enc(project)),
            Some(d),
        )
    }

    /// `DELETE /chains/{chain}`: the chain and its cards go; the answer says what became of the
    /// ships and run files. `project` narrows the chain's name to one project.
    pub fn delete_chain(&self, chain: &str, project: Option<&str>) -> ClientResult<ChainDeletion> {
        let mut path = format!("/chains/{}", enc(chain));
        if let Some(p) = project {
            path = format!("{path}?project={}", enc(p));
        }
        self.call("DELETE", &path, NO_BODY)
    }

    /// `POST /chains/{chain}/tickets/{ticket}/move`.
    pub fn move_ticket(
        &self,
        chain: &str,
        ticket: &str,
        m: &MoveInput,
    ) -> ClientResult<ChainDetail> {
        let path = format!("/chains/{}/tickets/{}/move", enc(chain), enc(ticket));
        self.call("POST", &path, Some(m))
    }

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
