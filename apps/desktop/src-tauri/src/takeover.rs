//! What the app does about a daemon that is already running. After a rebuild and reinstall the old
//! daemon (older version, or the binary of a replaced app) must give way to the app's own; a
//! daemon of the same version from the same binary is simply attached to.

/// The daemon found by `daemon status --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub version: Option<String>,
    pub exe: Option<String>,
    /// Agent slots in use; 0 when the daemon did not say.
    pub agents: u32,
}

/// What to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No daemon runs: start ours.
    Start,
    /// The same version from the same binary: use it.
    Attach,
    /// Another version or binary (or one that does not say): stop it, then start ours.
    Replace,
    /// As `Replace`, but this many agents are running: ask first, stopping the daemon stops them.
    Ask(u32),
}

/// Decides from what is running and the app's own version and daemon binary (both paths already
/// resolved the same way). A daemon that reports no binary (older than 0.2) is replaced.
pub fn decide(found: Option<&Found>, own_version: &str, own_exe: &str) -> Decision {
    let Some(f) = found else {
        return Decision::Start;
    };
    if f.version.as_deref() == Some(own_version) && f.exe.as_deref() == Some(own_exe) {
        Decision::Attach
    } else if f.agents > 0 {
        Decision::Ask(f.agents)
    } else {
        Decision::Replace
    }
}

/// The question in the window; "later" keeps the running daemon until no agent runs.
pub fn ask_message(found: &Found, agents: u32) -> String {
    let s = if agents == 1 { "" } else { "s" };
    format!(
        "An older Toknila daemon (version {}) is running with {agents} agent{s}.\n\n\
         Replace now stops the agent{s}. Later keeps using the running daemon, and Toknila \
         replaces it once no agent is running.",
        found.version.as_deref().unwrap_or("unknown"),
    )
}

/// The label of the replace-now button, with the number it stops.
pub fn replace_label(agents: u32) -> String {
    let s = if agents == 1 { "" } else { "s" };
    format!("Replace now (stops {agents} agent{s})")
}

/// What the start page shows: built from the old daemon's data, sent over IPC.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Question {
    pub agents: u32,
    pub message: String,
    pub replace_label: String,
}

impl Question {
    /// The question for a daemon `found` with `agents` live runners.
    pub fn new(found: &Found, agents: u32) -> Self {
        Self {
            agents,
            message: ask_message(found, agents),
            replace_label: replace_label(agents),
        }
    }
}

/// A script that puts a dismissible notice on the page the window now shows.
pub fn notice_script(text: &str) -> String {
    let text = serde_json::to_string(text).unwrap_or_default();
    format!(
        "(()=>{{const n=document.createElement('div');n.setAttribute('role','status');\
         n.style.cssText='position:fixed;left:16px;right:16px;bottom:16px;z-index:99999;\
         padding:12px 16px;border-radius:8px;background:#222;color:#fff;font:14px system-ui;\
         cursor:pointer';n.textContent={text};n.title='Click to dismiss';\
         n.onclick=()=>n.remove();document.body.appendChild(n);}})();"
    )
}

/// What the watcher found out about the daemon it waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// Agents still run: look again later.
    Busy,
    /// No agent runs, or the daemon is gone: replace it.
    Idle,
}

/// Reads `daemon status --json` for the "later" watcher.
pub fn wait_state(json: &str) -> Wait {
    match parse_status(json) {
        Some(f) if f.agents > 0 => Wait::Busy,
        _ => Wait::Idle,
    }
}

/// The notice shown when "later" has replaced the daemon.
pub const REPLACED_LATER: &str =
    "No agent is running any more, so Toknila replaced the older daemon with its own.";

/// Reads the answer of `toknila daemon status --json`: `None` when no daemon runs.
pub fn parse_status(json: &str) -> Option<Found> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v["running"].as_bool() != Some(true) {
        return None;
    }
    let text = |k: &str| v[k].as_str().map(str::to_owned);
    Some(Found {
        version: text("version"),
        exe: text("exe"),
        agents: v["agents"]
            .as_u64()
            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX)),
    })
}

/// The message in the window when the old daemon would not stop.
pub fn stuck_message(found: &Found, reason: &str) -> String {
    format!(
        "An older Toknila daemon (version {}, {}) is still running and did not stop: {reason}\n\
         Stop it with `tok daemon stop` (or quit its process) and open Toknila again.",
        found.version.as_deref().unwrap_or("unknown"),
        found.exe.as_deref().unwrap_or("unknown binary"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(v: Option<&str>, e: Option<&str>) -> Found {
        Found {
            version: v.map(str::to_owned),
            exe: e.map(str::to_owned),
            agents: 0,
        }
    }

    fn busy(n: u32) -> Found {
        Found {
            agents: n,
            ..found(Some("0.1.0"), Some("/a/toknila"))
        }
    }

    #[test]
    fn live_agents_make_replacing_a_question() {
        assert_eq!(
            decide(Some(&busy(3)), "0.2.0", "/a/toknila"),
            Decision::Ask(3)
        );
        // The same version and binary is attached to, agents or not.
        let same = Found {
            agents: 3,
            ..found(Some("0.2.0"), Some("/a/toknila"))
        };
        assert_eq!(decide(Some(&same), "0.2.0", "/a/toknila"), Decision::Attach);
    }

    #[test]
    fn the_question_names_the_count_and_the_two_ways() {
        assert_eq!(replace_label(2), "Replace now (stops 2 agents)");
        assert_eq!(replace_label(1), "Replace now (stops 1 agent)");
        let m = ask_message(&busy(2), 2);
        assert!(m.contains("2 agents") && m.contains("Later") && m.contains("once no agent"));
    }

    #[test]
    fn later_waits_while_agents_run_and_goes_when_idle_or_gone() {
        let on = |n: u32| format!(r#"{{"running":true,"version":"0.1.0","agents":{n}}}"#);
        assert_eq!(wait_state(&on(2)), Wait::Busy);
        assert_eq!(wait_state(&on(0)), Wait::Idle);
        assert_eq!(wait_state(r#"{"running":false}"#), Wait::Idle);
        let m = REPLACED_LATER;
        assert!(m.contains("replaced"));
    }

    #[test]
    fn the_page_gets_the_count_and_the_labels() {
        let q = Question::new(&busy(2), 2);
        assert_eq!(q.agents, 2);
        assert_eq!(q.replace_label, "Replace now (stops 2 agents)");
        let json = serde_json::to_string(&q).unwrap_or_default();
        assert!(json.contains(r#""agents":2"#), "{json}");
    }

    #[test]
    fn the_notice_script_carries_the_text_quoted() {
        let s = notice_script(r#"said "hi""#);
        assert!(s.contains(r#"textContent="said \"hi\"""#), "{s}");
    }

    #[test]
    fn status_reads_the_agent_count() {
        let on = r#"{"running":true,"version":"0.1.0","agents":4}"#;
        assert_eq!(parse_status(on).map(|f| f.agents), Some(4));
    }

    #[test]
    fn nothing_running_starts_ours() {
        assert_eq!(decide(None, "0.2.0", "/a/toknila"), Decision::Start);
    }

    #[test]
    fn same_version_and_binary_is_attached() {
        let f = found(Some("0.2.0"), Some("/a/toknila"));
        assert_eq!(decide(Some(&f), "0.2.0", "/a/toknila"), Decision::Attach);
    }

    #[test]
    fn another_version_another_path_or_an_unknown_one_is_replaced() {
        let own = ("0.2.0", "/a/toknila");
        for f in [
            found(Some("0.1.0"), Some("/a/toknila")),
            found(Some("0.2.0"), Some("/b/toknila")),
            found(Some("0.2.0"), None),
            found(None, Some("/a/toknila")),
        ] {
            assert_eq!(decide(Some(&f), own.0, own.1), Decision::Replace, "{f:?}");
        }
    }

    #[test]
    fn status_json_is_read_with_and_without_a_binary() {
        let on =
            r#"{"running":true,"pid":1,"port":2,"version":"0.2.0","exe":"/a/toknila","home":"/h"}"#;
        assert_eq!(
            parse_status(on),
            Some(found(Some("0.2.0"), Some("/a/toknila")))
        );
        let old = r#"{"running":true,"pid":1,"port":2,"version":"0.1.0","home":"/h"}"#;
        assert_eq!(parse_status(old), Some(found(Some("0.1.0"), None)));
        assert_eq!(parse_status(r#"{"running":false,"home":"/h"}"#), None);
        assert_eq!(parse_status("garbage"), None);
    }

    #[test]
    fn the_message_names_the_old_daemon_and_the_way_out() {
        let m = stuck_message(&found(Some("0.1.0"), None), "timed out");
        assert!(m.contains("version 0.1.0") && m.contains("unknown binary"));
        assert!(m.contains("tok daemon stop") && m.contains("timed out"));
    }
}
