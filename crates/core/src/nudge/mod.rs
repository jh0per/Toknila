//! The silence rule: when a quiet agent is nudged, and when it is given up on. Pure; the daemon
//! gathers the facts and runs what is decided.
//!
//! Where "quiet" is measured, in order of trust: the agent's own MCP calls, then its hooks, and
//! only when neither has ever been seen (beyond the run's start) the terminal's output. A person's
//! instruction delivered to the agent and a nudge both count as a sign of life, so neither is
//! followed at once by another. A live child process (a build, tests) means the agent is waiting
//! on work, not silent.

#[cfg(test)]
mod tests;

use crate::{Nudge, Timestamp};

/// Where a sign of life came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Mcp,
    Hook,
}

/// Which source a recorded activity signal (`api`, `hook:stop`, `start`) is. The run's own start
/// is no source.
pub fn source_of(signal: &str) -> Option<Source> {
    match signal {
        "api" => Some(Source::Mcp),
        s if s.starts_with("hook:") => Some(Source::Hook),
        _ => None,
    }
}

/// Whether an activity signal shows the agent working again, so a Waiting card goes back to
/// Running. A Stop or Notification hook fires when the agent stops to wait, so those do not.
pub fn resumes(signal: &str) -> bool {
    matches!(signal, "hook:post_tool_use" | "hook:user_prompt_submit")
}

/// What is known about a run's signs of life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signals {
    /// When the run started.
    pub started: Timestamp,
    /// Its last MCP call.
    pub mcp: Option<Timestamp>,
    /// Its last hook.
    pub hook: Option<Timestamp>,
    /// Its last output.
    pub output: Option<Timestamp>,
    /// When a person's instruction (or a nudge) last reached it.
    pub delivered: Option<Timestamp>,
}

impl Signals {
    /// The last moment the agent showed life, by the order of trust above.
    pub fn last(&self) -> Timestamp {
        let reported = self.mcp.max(self.hook);
        let base = reported.or(self.output).unwrap_or(self.started);
        base.max(self.delivered.unwrap_or(base))
    }
}

/// What to do about a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Nothing yet.
    Wait,
    /// Ask the agent to report its state.
    Nudge,
    /// Nudges did not help: the card waits for a person (silent).
    GiveUp,
}

/// Decides for one run. `nudges` are the times of the nudges already sent to it.
pub fn decide(
    cfg: &Nudge,
    signals: &Signals,
    child_alive: bool,
    nudges: &[Timestamp],
    now: Timestamp,
) -> Decision {
    if child_alive {
        return Decision::Wait;
    }
    let since = nudges
        .iter()
        .copied()
        .max()
        .map_or(signals.last(), |n| n.max(signals.last()));
    if now.millis_since(since) < cfg.idle_secs.saturating_mul(1000) {
        return Decision::Wait;
    }
    let from = now
        .0
        .saturating_sub(i64::try_from(cfg.window_secs.saturating_mul(1000)).unwrap_or(i64::MAX));
    let recent = nudges.iter().filter(|n| n.0 > from).count();
    if recent >= usize::try_from(cfg.max_nudges).unwrap_or(usize::MAX) {
        Decision::GiveUp
    } else {
        Decision::Nudge
    }
}
