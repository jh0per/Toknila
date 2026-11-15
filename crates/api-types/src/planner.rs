//! A project's planner as the window and the CLI see it: whether its process is alive, and
//! whether it works on a message and what it does now.

use serde::{Deserialize, Serialize};
use toknila_core::planner::turn::Signal;
use toknila_core::{RunId, Timestamp};

/// `GET /v1/projects/{project}/planner`: the project's planner.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerState {
    /// Its process is alive.
    pub running: bool,
    /// The live run; `null` when it is not running.
    pub run: Option<RunId>,
    /// The profile it runs with (or, when it is not running, would start with).
    pub profile: String,
    /// Its turn is on: it took a message and has not stopped. Never true when it is not running.
    #[serde(default)]
    pub working: bool,
    /// One line about what it does now ("Reading the plan"); `null` outside a turn.
    #[serde(default)]
    pub step: Option<String>,
    /// When its turn started; `null` outside a turn.
    #[serde(default)]
    pub since: Option<Timestamp>,
}

/// `POST /v1/projects/{project}/planner/start`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartPlanner {
    /// A planner profile from `GET /v1/profiles`; unset: `claude-code-planner`.
    pub profile: Option<String>,
}

/// `POST /v1/planner/activity`: what the planner's process reports about its turn, from a hook.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerActivity {
    /// The hook that fired: `user_prompt_submit` starts the turn, `pre_tool_use` names its step,
    /// `stop` ends it.
    pub signal: Signal,
    /// A `pre_tool_use`: the tool about to run (`Read`, `mcp__toknila__add_card`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// A `pre_tool_use`: the summary of the call (`Read: /repo/src/main.rs`), as
    /// `toknila_core::hook::tool_text` writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planner_state_and_the_activity_are_pinned_on_the_wire() {
        let run: RunId = "01JBQ7ZJ0000000000000000R1".parse().expect("run");
        let state = PlannerState {
            running: true,
            run: Some(run),
            profile: "claude-code-planner".to_owned(),
            working: true,
            step: Some("Reading the plan".to_owned()),
            since: Some(Timestamp(5)),
        };
        assert_eq!(
            serde_json::to_string(&state).expect("json"),
            r#"{"running":true,"run":"01JBQ7ZJ0000000000000000R1","profile":"claude-code-planner","working":true,"step":"Reading the plan","since":5}"#
        );
        // An older daemon's answer has no turn: it reads as not working.
        let old: PlannerState =
            serde_json::from_str(r#"{"running":true,"run":null,"profile":"p"}"#).expect("old");
        assert_eq!((old.working, old.step, old.since), (false, None, None));

        let a = PlannerActivity {
            signal: Signal::PreToolUse,
            tool_name: Some("Read".to_owned()),
            tool: Some("Read: /r/a.rs".to_owned()),
        };
        assert_eq!(
            serde_json::to_string(&a).expect("json"),
            r#"{"signal":"pre_tool_use","tool_name":"Read","tool":"Read: /r/a.rs"}"#
        );
        let bare = PlannerActivity {
            signal: Signal::Stop,
            tool_name: None,
            tool: None,
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("json"),
            r#"{"signal":"stop"}"#
        );
    }
}
