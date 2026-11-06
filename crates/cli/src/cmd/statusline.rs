//! `tok statusline`: the status line command of an agent run. Claude Code hands it a JSON document
//! on stdin whose `rate_limits` field (Pro and Max subscribers) holds the plan usage; it is passed
//! to the daemon for the Usage page. The line itself stays empty, and nothing here may get in the
//! agent's way: failures are a line on stderr and exit code 0.

use std::io::Read;

use anyhow::Result;
use toknila_api_types::QuotaReport;
use toknila_core::usage::quota;

use super::agent::AgentEnv;

/// What to report for a status line's input: the documented rate-limit windows, if it has any.
fn report_for(stdin: &str) -> Option<QuotaReport> {
    let windows = quota::parse_status_line(stdin);
    (!windows.is_empty()).then_some(QuotaReport { windows })
}

pub(super) fn run() -> Result<()> {
    // Outside a Toknila run (the user's own session) there is nothing to tell.
    let Ok(agent) = AgentEnv::from_env() else {
        return Ok(());
    };
    let mut stdin = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut stdin);
    if let Some(report) = report_for(&stdin)
        && let Err(e) = agent.client.report_quota(&report)
    {
        eprintln!("toknila statusline: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limits_are_reported_and_other_input_is_not() {
        let json = r#"{"model":{"display_name":"x"},"rate_limits":{"five_hour":{"used_percentage":12.5,"resets_at":1738425600}}}"#;
        let report = report_for(json).expect("report");
        assert_eq!(report.windows.len(), 1);
        assert_eq!(report.windows[0].name, "five_hour");
        assert_eq!(report.windows[0].used_percentage, 12.5);
        assert_eq!(report_for(r#"{"model":{}}"#), None);
        assert_eq!(report_for("garbage"), None);
        assert_eq!(report_for(""), None);
    }
}
