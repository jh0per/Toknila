//! The usage routes.

use toknila_api_types::{QuotaReport, UsageView};

use crate::{Client, ClientResult};

impl Client {
    /// `GET /usage?days=`: totals, series and breakdowns of the last `days` days.
    pub fn usage(&self, days: u32) -> ClientResult<UsageView> {
        self.call("GET", &format!("/usage?days={days}"), crate::NO_BODY)
    }

    /// `POST /usage/quota`: the plan limits an agent's status line received.
    pub fn report_quota(&self, q: &QuotaReport) -> ClientResult<()> {
        self.call("POST", "/usage/quota", Some(q))
    }
}
