//! The review settings routes of chains.

use toknila_api_types::{ChainDetail, ReviewSettings};

use crate::{Client, ClientResult, enc};

impl Client {
    /// `PATCH /chains/{chain}`: how the chain's tickets are reviewed.
    pub fn patch_chain(&self, chain: &str, s: &ReviewSettings) -> ClientResult<ChainDetail> {
        self.call("PATCH", &format!("/chains/{}", enc(chain)), Some(s))
    }
}
