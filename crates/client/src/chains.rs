//! The chain routes: create, import, export, read, change and delete a chain, its
//! dependencies and the order of its cards.

use crate::{Client, ClientResult, NO_BODY, enc};
use toknila_api_types::{
    ChainDeletion, ChainDetail, ChainFile, ChainItem, ChainSettings, EdgeInput, EdgeMoveInput,
    MoveChain, MoveInput, NewChain,
};

impl Client {
    /// `POST /chains/{chain}/move`: the chain goes before or after another of its project;
    /// `project` narrows the name.
    pub fn move_chain(
        &self,
        chain: &str,
        project: Option<&str>,
        to: &MoveChain,
    ) -> ClientResult<ChainDetail> {
        let mut path = format!("/chains/{}/move", enc(chain));
        if let Some(p) = project {
            path = format!("{path}?project={}", enc(p));
        }
        self.call("POST", &path, Some(to))
    }

    /// `POST /projects/{project}/chains`.
    pub fn create_chain(&self, project: &str, c: &NewChain) -> ClientResult<ChainDetail> {
        self.call(
            "POST",
            &format!("/projects/{}/chains", enc(project)),
            Some(c),
        )
    }

    /// `POST /projects/{project}/import`.
    pub fn import_chain(&self, project: &str, toml: String) -> ClientResult<ChainDetail> {
        let path = format!("/projects/{}/import", enc(project));
        self.call("POST", &path, Some(&ChainFile { toml }))
    }

    /// `GET /chains/{chain}/export`.
    pub fn export_chain(&self, chain: &str) -> ClientResult<String> {
        let path = format!("/chains/{}/export", enc(chain));
        Ok(self.call::<ChainFile>("GET", &path, NO_BODY)?.toml)
    }

    /// `GET /chains`.
    pub fn chains(&self) -> ClientResult<Vec<ChainItem>> {
        self.call("GET", "/chains", NO_BODY)
    }

    /// `GET /chains/{chain}`.
    pub fn chain(&self, chain: &str) -> ClientResult<ChainDetail> {
        self.call("GET", &format!("/chains/{}", enc(chain)), NO_BODY)
    }

    /// `POST /chains/{chain}/edges`.
    pub fn add_edge(&self, chain: &str, e: &EdgeInput) -> ClientResult<ChainDetail> {
        self.call("POST", &format!("/chains/{}/edges", enc(chain)), Some(e))
    }

    /// `DELETE /chains/{chain}/edges`.
    pub fn remove_edge(&self, chain: &str, from: &str, to: &str) -> ClientResult<ChainDetail> {
        let path = format!(
            "/chains/{}/edges?from={}&to={}",
            enc(chain),
            enc(from),
            enc(to)
        );
        self.call("DELETE", &path, NO_BODY)
    }

    /// `POST /chains/{chain}/edges/move`.
    pub fn move_edge(&self, chain: &str, m: &EdgeMoveInput) -> ClientResult<ChainDetail> {
        self.call(
            "POST",
            &format!("/chains/{}/edges/move", enc(chain)),
            Some(m),
        )
    }

    /// `PATCH /chains/{chain}`: review settings and autostart; `project` narrows the name.
    pub fn patch_chain(
        &self,
        chain: &str,
        project: Option<&str>,
        s: &ChainSettings,
    ) -> ClientResult<ChainDetail> {
        let mut path = format!("/chains/{}", enc(chain));
        if let Some(p) = project {
            path = format!("{path}?project={}", enc(p));
        }
        self.call("PATCH", &path, Some(s))
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
}
