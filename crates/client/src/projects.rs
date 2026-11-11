//! The project routes: create, read, change, archive and delete a project.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{DeleteProject, NewProject, PatchProject, ProjectDeletion};
use toknila_core::Project;

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
}

impl Client {
    /// `GET /projects/{project}/strays`: the leftover ships no live card owns.
    pub fn strays(&self, project: &str) -> ClientResult<toknila_api_types::Strays> {
        self.call(
            "GET",
            &format!("/projects/{}/strays", enc(project)),
            NO_BODY,
        )
    }

    /// `POST /projects/{project}/strays/clean`: delete them.
    pub fn clean_strays(&self, project: &str) -> ClientResult<toknila_api_types::StraysCleaned> {
        let path = format!("/projects/{}/strays/clean", enc(project));
        self.call("POST", &path, Some(&serde_json::json!({})))
    }
}
