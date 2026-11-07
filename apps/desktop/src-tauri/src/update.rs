//! What the update feature decides, as pure functions: when to check, what the window shows,
//! whether a restart must warn first. No I/O here; `updater.rs` does the downloading and calls
//! these.

use serde::Serialize;
use std::time::Duration;

/// How often the app asks for a newer version (the first check is at start).
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Longest release note the banner carries, in characters.
const NOTES_MAX_CHARS: usize = 240;

/// Most lines of the release notes the banner carries.
const NOTES_MAX_LINES: usize = 4;

/// Where an update is in its life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Nothing newer is known.
    Idle,
    /// Asking the release server.
    Checking,
    /// A newer version was found and is downloading in the background.
    Downloading(String),
    /// Downloaded and verified; installing needs a restart.
    Ready { version: String, notes: String },
}

/// Everything the window may show about updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The running version.
    pub current_version: String,
    /// Unix seconds of the last finished check, if any.
    pub last_check: Option<u64>,
    /// Where the update is.
    pub phase: Phase,
    /// The last check's failure, cleared by the next success.
    pub error: Option<String>,
}

/// The quiet banner: shown only when an update is ready to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Banner {
    /// "Toknila X.Y.Z is ready".
    pub title: String,
    /// A short release note, possibly empty.
    pub notes: String,
}

/// The view the page renders (the page decides nothing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// The running version.
    pub current_version: String,
    /// Unix seconds of the last finished check.
    pub last_check: Option<u64>,
    /// A check or a download is under way.
    pub busy: bool,
    /// The banner, when an update is ready.
    pub banner: Option<Banner>,
    /// The last failure, in words.
    pub error: Option<String>,
}

/// How long to wait before the next check: none at start (never checked), then what is left of
/// [`CHECK_EVERY`] since the last one.
pub fn wait_before_next(now: u64, last_check: Option<u64>) -> Duration {
    match last_check {
        None => Duration::ZERO,
        Some(t) => CHECK_EVERY.saturating_sub(Duration::from_secs(now.saturating_sub(t))),
    }
}

/// The first lines of the release notes, short enough for a banner.
pub fn short_notes(notes: &str) -> String {
    let lines: Vec<&str> = notes
        .lines()
        .map(|l| l.trim().trim_start_matches("- "))
        .filter(|l| !l.is_empty())
        .take(NOTES_MAX_LINES)
        .collect();
    let text = lines.join("\n");
    if text.chars().count() <= NOTES_MAX_CHARS {
        return text;
    }
    let cut: String = text.chars().take(NOTES_MAX_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// What the window shows for a status.
pub fn view(status: &Status) -> View {
    let banner = match &status.phase {
        Phase::Ready { version, notes } => Some(Banner {
            title: format!("Toknila {version} is ready"),
            notes: short_notes(notes),
        }),
        _ => None,
    };
    View {
        current_version: status.current_version.clone(),
        last_check: status.last_check,
        busy: matches!(status.phase, Phase::Checking | Phase::Downloading(_)),
        banner,
        error: status.error.clone(),
    }
}

/// What a restart request leads to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Gate {
    /// Install and restart now.
    Go,
    /// Ask first: agents would stop. The page offers "Restart anyway" and "Later".
    Warn {
        /// The sentence to show.
        message: String,
    },
}

/// Whether a restart must warn. `running` is the number of agents working now (`None` when the
/// daemon could not be asked: then it is not known that nothing would stop). A restart the person
/// already confirmed goes ahead; one with agents running never does without that.
pub fn restart_gate(running: Option<usize>, confirmed: bool) -> Gate {
    match running {
        Some(0) => Gate::Go,
        _ if confirmed => Gate::Go,
        Some(n) => Gate::Warn {
            message: if n == 1 {
                "1 agent is running; restarting stops it".to_owned()
            } else {
                format!("{n} agents are running; restarting stops them")
            },
        },
        None => Gate::Warn {
            message: "Cannot tell whether agents are running; restarting may stop them".to_owned(),
        },
    }
}

/// The number of running tickets in the JSON of `tok ls --json`.
pub fn count_running(json: &str) -> Option<usize> {
    let items: serde_json::Value = serde_json::from_str(json).ok()?;
    let items = items.as_array()?;
    Some(
        items
            .iter()
            .filter(|t| t["ticket"]["status"].as_str() == Some("running"))
            .count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u64 = 3600;

    #[test]
    fn checks_at_start_then_every_six_hours() {
        assert_eq!(
            wait_before_next(1000 + 6 * H - 1, Some(1000)),
            Duration::from_secs(1)
        );
        assert_eq!(wait_before_next(1000 + 6 * H, Some(1000)), Duration::ZERO);
        // A clock that went back waits at most one period.
        assert_eq!(wait_before_next(500, Some(1000)), CHECK_EVERY);
        assert_eq!(wait_before_next(1000, None), Duration::ZERO);
        assert_eq!(
            wait_before_next(1000 + 2 * H, Some(1000)),
            Duration::from_secs(4 * H)
        );
        assert_eq!(wait_before_next(1000 + 9 * H, Some(1000)), Duration::ZERO);
    }

    fn status(phase: Phase) -> Status {
        Status {
            current_version: "0.2.0".to_owned(),
            last_check: Some(7),
            phase,
            error: None,
        }
    }

    #[test]
    fn the_banner_shows_only_for_a_ready_update() {
        let ready = Phase::Ready {
            version: "0.2.1".to_owned(),
            notes: "Quieter cards\n\nFaster start".to_owned(),
        };
        let v = view(&status(ready));
        assert_eq!(
            v.banner,
            Some(Banner {
                title: "Toknila 0.2.1 is ready".to_owned(),
                notes: "Quieter cards\nFaster start".to_owned(),
            })
        );
        assert!(!v.busy);
        for phase in [
            Phase::Idle,
            Phase::Checking,
            Phase::Downloading("0.2.1".to_owned()),
        ] {
            assert_eq!(view(&status(phase)).banner, None);
        }
        assert!(view(&status(Phase::Checking)).busy);
        assert!(view(&status(Phase::Downloading("0.2.1".to_owned()))).busy);
        assert!(!view(&status(Phase::Idle)).busy);
        assert_eq!(view(&status(Phase::Idle)).last_check, Some(7));
    }

    #[test]
    fn notes_are_cut_short() {
        assert_eq!(short_notes("  \n"), "");
        assert_eq!(short_notes("a\nb\nc\nd\ne"), "a\nb\nc\nd");
        // The release's Markdown bullets read as plain lines.
        assert_eq!(short_notes("- one\n- two"), "one\ntwo");
        let long = "x".repeat(500);
        let s = short_notes(&long);
        assert_eq!(s.chars().count(), NOTES_MAX_CHARS + 1);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn restart_warns_while_agents_run() {
        assert_eq!(restart_gate(Some(0), false), Gate::Go);
        assert_eq!(
            restart_gate(Some(3), false),
            Gate::Warn {
                message: "3 agents are running; restarting stops them".to_owned()
            }
        );
        assert_eq!(
            restart_gate(Some(1), false),
            Gate::Warn {
                message: "1 agent is running; restarting stops it".to_owned()
            }
        );
        // Only an explicit confirmation lets a restart stop agents.
        assert_eq!(restart_gate(Some(3), true), Gate::Go);
        // Not knowing is not the same as none running.
        assert!(matches!(restart_gate(None, false), Gate::Warn { .. }));
        assert_eq!(restart_gate(None, true), Gate::Go);
    }

    #[test]
    fn counts_running_tickets() {
        let json = r#"[{"ticket":{"key":"A-1","status":"running"}},{"ticket":{"key":"A-2","status":"done"}},{"ticket":{"key":"A-3","status":"running"}}]"#;
        assert_eq!(count_running(json), Some(2));
        assert_eq!(count_running("[]"), Some(0));
        assert_eq!(count_running("not json"), None);
        assert_eq!(count_running("{}"), None);
    }
}
