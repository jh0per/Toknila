//! The remaining plan quota, as far as Claude Code documents it: the `rate_limits` field of the
//! JSON a status line command receives (subscribers only, after a session's first reply).
//! Nothing else is read, and no credential is touched.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// One rate-limit window.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    /// `five_hour` or `seven_day`.
    pub name: String,
    /// 0 to 100 (above 100 once exceeded).
    pub used_percentage: f64,
    /// Unix epoch seconds when the window resets.
    pub resets_at: i64,
}

/// What a status line last reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub seen_at: Timestamp,
    pub windows: Vec<QuotaWindow>,
}

/// What the Usage page shows about the quota.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaView {
    pub available: bool,
    /// Why there is nothing to show, or where the numbers come from.
    pub reason: String,
    pub seen_at: Option<Timestamp>,
    pub windows: Vec<QuotaWindow>,
}

/// The windows in a status line's JSON input.
pub fn parse_status_line(json: &str) -> Vec<QuotaWindow> {
    #[derive(Deserialize)]
    struct Window {
        used_percentage: f64,
        resets_at: i64,
    }
    #[derive(Deserialize, Default)]
    struct Limits {
        five_hour: Option<Window>,
        seven_day: Option<Window>,
    }
    #[derive(Deserialize)]
    struct Input {
        #[serde(default)]
        rate_limits: Limits,
    }
    let Ok(input) = serde_json::from_str::<Input>(json) else {
        return vec![];
    };
    [
        ("five_hour", input.rate_limits.five_hour),
        ("seven_day", input.rate_limits.seven_day),
    ]
    .into_iter()
    .filter_map(|(name, w)| {
        w.map(|w| QuotaWindow {
            name: name.to_owned(),
            used_percentage: w.used_percentage,
            resets_at: w.resets_at,
        })
    })
    .collect()
}

const SOURCE: &str =
    "Reported by Claude Code's status line (rate_limits), as of the last agent reply.";
const MISSING: &str = "Not available: Claude Code reports plan limits only to a status line command, \
only for Pro and Max subscribers, and only after an agent's first reply. None has reported yet.";

/// The quota to show at `now`: windows that have reset are no longer true and are dropped.
pub fn view(snapshot: Option<&QuotaSnapshot>, now: Timestamp) -> QuotaView {
    let windows: Vec<QuotaWindow> = snapshot
        .map(|s| {
            s.windows
                .iter()
                .filter(|w| w.resets_at.saturating_mul(1000) > now.0)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    QuotaView {
        available: !windows.is_empty(),
        reason: if windows.is_empty() { MISSING } else { SOURCE }.to_owned(),
        seen_at: snapshot.map(|s| s.seen_at).filter(|_| !windows.is_empty()),
        windows,
    }
}
