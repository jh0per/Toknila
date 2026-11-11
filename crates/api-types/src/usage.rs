//! Bodies of the usage routes. Numbers only: no prompt or message content ever crosses here.

use serde::{Deserialize, Serialize};
use toknila_core::usage::quota::{QuotaView, QuotaWindow};
use toknila_core::usage::{PriceTable, UsageSummary};

/// Where the cost estimate's prices come from.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceInfo {
    /// The page the shipped prices were taken from.
    pub source: String,
    /// The day they were taken (`YYYY-MM-DD`).
    pub retrieved: String,
    /// Models whose price the person overrode in `usage-prices.toml`.
    pub overridden: Vec<String>,
    /// Why `usage-prices.toml` was not used, when it could not be read.
    pub problem: Option<String>,
}

impl PriceInfo {
    /// The origin of the prices in use: the shipped table and a person's overrides.
    pub fn new(shipped: &PriceTable, overrides: &PriceTable) -> Self {
        Self {
            source: shipped.source.clone(),
            retrieved: shipped.retrieved.clone(),
            overridden: overrides.models.iter().map(|m| m.id.clone()).collect(),
            problem: None,
        }
    }
}

/// `GET /v1/usage`: the usage page in one answer. Every cost in it is an estimate at API list
/// prices; on a subscription the real cost differs.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageView {
    pub summary: UsageSummary,
    /// Whether every Claude Code session on this machine is counted, not only Toknila's runs.
    pub machine_scan: bool,
    pub prices: PriceInfo,
    /// The remaining plan quota, when Claude Code reported one.
    pub quota: QuotaView,
}

/// Body of `PUT /v1/usage/settings`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageSettings {
    /// Count every Claude Code session under the user's `~/.claude/projects`, not only
    /// Toknila's runs. Local only; on by default.
    pub machine_scan: bool,
}

/// Body of `POST /v1/usage/quota`: the rate-limit windows an agent's status line received.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaReport {
    pub windows: Vec<QuotaWindow>,
}
