//! The settings route of chains.

use toknila_api_types::{ChainDetail, ChainSettings};

use crate::{Client, ClientResult, enc};

impl Client {
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
}
