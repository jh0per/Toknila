//! The usage routes.

use toknila_api_types::{QuotaReport, UsageSettings, UsageView};

use crate::{Client, ClientResult};

impl Client {
    /// `GET /usage?days=`: totals, series and breakdowns of the last `days` days.
    pub fn usage(&self, days: u32) -> ClientResult<UsageView> {
        self.call("GET", &format!("/usage?days={days}"), crate::NO_BODY)
    }

    /// `PUT /usage/settings`: count every session on this machine, or only Toknila's runs.
    pub fn set_usage_settings(&self, s: &UsageSettings) -> ClientResult<UsageSettings> {
        self.call("PUT", "/usage/settings", Some(s))
    }

    /// `POST /usage/quota`: the plan limits an agent's status line received.
    pub fn report_quota(&self, q: &QuotaReport) -> ClientResult<()> {
        self.call("POST", "/usage/quota", Some(q))
    }
}
