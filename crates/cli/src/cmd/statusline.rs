//! `tok statusline`: the status line command of an agent run. Claude Code hands it a JSON document
//! on stdin whose `rate_limits` field (Pro and Max subscribers) holds the plan usage; it is passed
//! to the daemon for the Usage page. The line itself is the model and the context use, and nothing here may get in the
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

/// The line to print: the model's name and the share of the context window used, whichever the
/// input has (`model.display_name`, `context_window.used_percentage`). Empty when it has neither.
fn line_for(stdin: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdin) else {
        return String::new();
    };
    let model = value["model"]["display_name"]
        .as_str()
        .filter(|m| !m.is_empty());
    let used = value["context_window"]["used_percentage"].as_f64();
    match (model, used) {
        (Some(m), Some(p)) => format!("{m} · {}% context", p.round()),
        (Some(m), None) => m.to_owned(),
        (None, Some(p)) => format!("{}% context", p.round()),
        (None, None) => String::new(),
    }
}

pub(super) fn run() -> Result<()> {
    // Outside a Toknila run (the user's own session) there is nothing to tell.
    let Ok(agent) = AgentEnv::from_env() else {
        return Ok(());
    };
    let mut stdin = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut stdin);
    println!("{}", line_for(&stdin));
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
    fn the_line_has_model_and_context_percentage() {
        let both = r#"{"model":{"display_name":"Opus"},"context_window":{"used_percentage":42.4}}"#;
        assert_eq!(line_for(both), "Opus · 42% context");
        assert_eq!(line_for(r#"{"model":{"display_name":"Opus"}}"#), "Opus");
        assert_eq!(
            line_for(r#"{"context_window":{"used_percentage":7}}"#),
            "7% context"
        );
        assert_eq!(line_for("garbage"), "");
    }

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
