//! `tok hook <name>`: a Claude Code hook as a second source of agent signals. The hook's JSON
//! arrives on stdin. Every hook is recorded as activity; a `PreToolUse` carries the summary of the
//! tool call, a permission prompt notification carries its message, and the `PermissionRequest`
//! hook holds the agent's dialog until the person decides in the window. What each means is
//! core's (`toknila_core::hook`).
//!
//! In a project's planner run (`TOKNILA_ROLE=planner`) a hook is a signal of the planner's turn
//! and nothing else: it posts `POST /v1/planner/activity` within [`PLANNER_DEADLINE`], prints
//! nothing, claims no instructions, holds no dialog and asks no pre-tool decision.
//!
//! A hook must never get in the agent's way: whatever goes wrong here is a line on stderr and
//! exit code 0 (Claude Code treats exit code 2 as "block this action").

use std::io::Read;
use std::time::Duration;

use anyhow::{Result, bail};
use toknila_api_types::{
    InstructionClaim, NewActivity, PermissionAsk, PermissionDecision, PermissionReply,
    PlannerActivity,
};
use toknila_core::AgentRole;
use toknila_core::hook::{self, HookInput};
use toknila_core::planner::turn::Signal;

use super::agent::AgentEnv;

/// How long a planner's hook waits for the daemon: the assistant waits on its hooks, so a slow
/// daemon costs it this and no more.
const PLANNER_DEADLINE: Duration = Duration::from_secs(2);

/// What a planner's hook `name` reports: its turn signal, with the tool's name and the summary of
/// the call for a `PreToolUse`. `None` for a hook a planner does not subscribe to.
fn planner_activity(name: &str, stdin: &str) -> Option<PlannerActivity> {
    let signal = hook::planner_signal(name)?;
    // A payload this version does not understand is still the signal.
    let input: HookInput = serde_json::from_str(stdin).unwrap_or_default();
    let step = signal == Signal::PreToolUse;
    Some(PlannerActivity {
        signal,
        tool_name: input
            .tool_name
            .clone()
            .filter(|n| step && !n.trim().is_empty()),
        tool: step.then(|| hook::tool_text(&input)).flatten(),
    })
}

/// A planner's hook: posts its signal on a thread of its own and waits [`PLANNER_DEADLINE`] at
/// most (the thread ends with the process). A failure is one line on stderr; stdout stays empty.
fn planner(name: &str, agent: AgentEnv, stdin: &str) {
    let Some(activity) = planner_activity(name, stdin) else {
        return;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(agent.client.planner_activity(&activity));
    });
    let failure = match rx.recv_timeout(PLANNER_DEADLINE) {
        Ok(Ok(())) => return,
        Ok(Err(e)) => e.to_string(),
        Err(_) => format!(
            "the daemon did not answer within {} s",
            PLANNER_DEADLINE.as_secs()
        ),
    };
    let line = failure.split_whitespace().collect::<Vec<_>>().join(" ");
    eprintln!("toknila hook {name}: {line}");
}

/// What a hook says, from its name and its stdin: the activity to record, with the session it fired
/// in (for the usage), the tool summary of a `PreToolUse`, and the message of a permission prompt.
fn signals(name: &str, stdin: &str) -> NewActivity {
    // A hook payload this version does not understand is still a sign of life.
    let input: HookInput = serde_json::from_str(stdin).unwrap_or_default();
    NewActivity {
        signal: format!("hook:{name}"),
        session_id: input.session_id.clone(),
        transcript_path: input.transcript_path.clone(),
        tool: matches!(name, "pre_tool_use" | "post_tool_use")
            .then(|| hook::tool_text(&input))
            .flatten(),
        prompt: hook::permission_prompt(name, &input),
    }
}

/// What a Stop hook prints to keep the agent working: the instructions a person left, as Claude
/// Code's block decision. Nothing when none were pending, so the agent stops. That also ends the
/// loop when the stop was itself caused by this hook (`stop_hook_active`): a claim hands each
/// instruction out once, so the next stop finds nothing.
fn stop_output(claim: &InstructionClaim) -> Option<String> {
    (claim.count > 0)
        .then(|| serde_json::json!({ "decision": "block", "reason": claim.reason }).to_string())
}

/// What a `PermissionRequest` hook prints for the person's decision: Claude Code's allow or deny
/// decision, with the person's reason for a denial. Nothing when nobody decided, so the agent
/// shows its own dialog.
fn permission_output(reply: &PermissionReply) -> Option<String> {
    let decision = match reply.decision {
        PermissionDecision::Allow => serde_json::json!({ "behavior": "allow" }),
        PermissionDecision::Deny => serde_json::json!({
            "behavior": "deny",
            "message": reply.message.clone().unwrap_or_default(),
        }),
        PermissionDecision::Ask => return None,
    };
    Some(
        serde_json::json!({
            "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision }
        })
        .to_string(),
    )
}

/// What a `PreToolUse` hook prints for a refusal: Claude Code's deny decision with the reason.
/// Nothing for any other reply, so the call goes on as the agent's permission mode says.
fn pre_tool_output(reply: &PermissionReply) -> Option<String> {
    (reply.decision == PermissionDecision::Deny).then(|| {
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reply.message.clone().unwrap_or_default(),
            }
        })
        .to_string()
    })
}

pub(super) fn run(name: &str) -> Result<()> {
    if !hook::is_known(name) {
        let known: Vec<&str> = hook::HOOKS.iter().map(|(_, ours)| *ours).collect();
        bail!("unknown hook {name:?}; known: {}", known.join(", "));
    }
    // Outside a Toknila run (the user's own session) there is nothing to tell.
    let Ok(agent) = AgentEnv::from_env() else {
        return Ok(());
    };
    let mut stdin = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut stdin);
    if agent.role == AgentRole::Planner {
        planner(name, agent, &stdin);
        return Ok(());
    }
    let activity = signals(name, &stdin);
    if let Err(e) = agent.client.activity(&agent.ticket, &activity) {
        eprintln!("toknila hook {name}: {e}");
    }
    if name == "stop" {
        match agent.client.claim_instructions(&agent.ticket) {
            Ok(claim) => {
                if let Some(out) = stop_output(&claim) {
                    println!("{out}");
                }
            }
            Err(e) => eprintln!("toknila hook {name}: {e}"),
        }
    }
    // Only a shell command can be a shell edit; any error or timeout prints nothing, so a hook
    // never blocks by accident.
    if name == "pre_tool_use"
        && activity
            .tool
            .as_deref()
            .is_some_and(|t| t.starts_with("Bash"))
    {
        let ask = PermissionAsk {
            tool: activity.tool.clone(),
        };
        match agent
            .client
            .pre_tool(&agent.ticket, &ask, Duration::from_secs(5))
        {
            Ok(reply) => {
                if let Some(out) = pre_tool_output(&reply) {
                    println!("{out}");
                }
            }
            Err(e) => eprintln!("toknila hook {name}: {e}"),
        }
    }
    if name == "permission_request" {
        let input: HookInput = serde_json::from_str(&stdin).unwrap_or_default();
        let ask = PermissionAsk {
            tool: hook::tool_text(&input),
        };
        let wait = Duration::from_secs(hook::PERMISSION_WAIT_SECS + 10);
        match agent.client.permission(&agent.ticket, &ask, wait) {
            Ok(reply) => {
                if let Some(out) = permission_output(&reply) {
                    println!("{out}");
                }
            }
            Err(e) => eprintln!("toknila hook {name}: {e}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_with_instructions_blocks_with_them_and_a_stop_without_prints_nothing() {
        let claim = InstructionClaim {
            count: 1,
            reason: "# [instruction] use rustls".to_owned(),
        };
        let out = stop_output(&claim).expect("block");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["decision"], "block");
        assert_eq!(v["reason"], "# [instruction] use rustls");
        let none = InstructionClaim {
            count: 0,
            reason: String::new(),
        };
        assert_eq!(stop_output(&none), None);
    }

    #[test]
    fn a_permission_prompt_is_activity_with_its_message() {
        let json = r#"{"hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow Bash?"}"#;
        let a = signals("notification", json);
        assert_eq!(a.signal, "hook:notification");
        assert_eq!(a.prompt.as_deref(), Some("Allow Bash?"));
        assert_eq!(a.tool, None);
    }

    #[test]
    fn a_pre_tool_use_carries_the_summary_of_the_call_and_no_contents() {
        let json = r#"{"hook_event_name":"PreToolUse","tool_name":"Write","tool_input":{"file_path":"/w/a.rs","content":"SECRET"}}"#;
        let a = signals("pre_tool_use", json);
        assert_eq!(a.signal, "hook:pre_tool_use");
        assert_eq!(a.tool.as_deref(), Some("Write: /w/a.rs"));
        assert_eq!(a.prompt, None);
        let wire = serde_json::to_string(&a).expect("json");
        assert!(!wire.contains("SECRET"), "{wire}");
    }

    #[test]
    fn stop_and_garbage_are_activity_only() {
        assert_eq!(
            signals("stop", r#"{"hook_event_name":"Stop"}"#),
            NewActivity::signal("hook:stop")
        );
        assert_eq!(
            signals("notification", "not json"),
            NewActivity::signal("hook:notification")
        );
        assert_eq!(signals("stop", ""), NewActivity::signal("hook:stop"));
    }

    #[test]
    fn the_session_and_its_transcript_travel_with_the_activity() {
        let json = r#"{"hook_event_name":"Stop","session_id":"abc","transcript_path":"/h/.claude/projects/p/abc.jsonl"}"#;
        let a = signals("stop", json);
        assert_eq!(a.session_id.as_deref(), Some("abc"));
        assert_eq!(
            a.transcript_path.as_deref(),
            Some("/h/.claude/projects/p/abc.jsonl")
        );
    }

    #[test]
    fn the_permission_hook_prints_claude_codes_decision_and_nothing_when_nobody_decided() {
        let reply = |decision, message: Option<&str>| PermissionReply {
            decision,
            message: message.map(str::to_owned),
        };
        let allow = permission_output(&reply(PermissionDecision::Allow, None)).expect("allow");
        let v: serde_json::Value = serde_json::from_str(&allow).expect("json");
        assert_eq!(
            v,
            serde_json::json!({"hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {"behavior": "allow"}
            }})
        );
        let deny =
            permission_output(&reply(PermissionDecision::Deny, Some("not that"))).expect("deny");
        let v: serde_json::Value = serde_json::from_str(&deny).expect("json");
        assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(v["hookSpecificOutput"]["decision"]["message"], "not that");
        assert_eq!(
            permission_output(&reply(PermissionDecision::Ask, None)),
            None
        );
    }

    #[test]
    fn the_pre_tool_hook_prints_claude_codes_deny_and_nothing_otherwise() {
        let reply = |decision, message: Option<&str>| PermissionReply {
            decision,
            message: message.map(str::to_owned),
        };
        let deny = pre_tool_output(&reply(PermissionDecision::Deny, Some("no shell edits")))
            .expect("deny");
        let v: serde_json::Value = serde_json::from_str(&deny).expect("json");
        assert_eq!(
            v,
            serde_json::json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "no shell edits"
            }})
        );
        assert_eq!(pre_tool_output(&reply(PermissionDecision::Ask, None)), None);
        assert_eq!(
            pre_tool_output(&reply(PermissionDecision::Allow, None)),
            None
        );
    }

    #[test]
    fn a_planners_pre_tool_use_is_its_step_with_the_tool_and_the_summary() {
        let json = r#"{"hook_event_name":"PreToolUse","session_id":"s","tool_name":"Read","tool_input":{"file_path":"/r/src/main.rs"}}"#;
        let a = planner_activity("pre_tool_use", json).expect("a planner signal");
        assert_eq!(
            serde_json::to_string(&a).expect("json"),
            r#"{"signal":"pre_tool_use","tool_name":"Read","tool":"Read: /r/src/main.rs"}"#
        );
        // A payload it does not understand is still the step's signal.
        let bare = planner_activity("pre_tool_use", "not json").expect("signal");
        assert_eq!(
            serde_json::to_string(&bare).expect("json"),
            r#"{"signal":"pre_tool_use"}"#
        );
    }

    #[test]
    fn a_planners_prompt_and_stop_are_bare_signals_and_other_hooks_none() {
        let start = planner_activity(
            "user_prompt_submit",
            r#"{"prompt":"plan","tool_name":"Read"}"#,
        );
        assert_eq!(
            serde_json::to_string(&start).expect("json"),
            r#"{"signal":"user_prompt_submit"}"#
        );
        let stop = planner_activity("stop", r#"{"stop_hook_active":false}"#);
        assert_eq!(
            serde_json::to_string(&stop).expect("json"),
            r#"{"signal":"stop"}"#
        );
        for other in [
            "permission_request",
            "notification",
            "post_tool_use",
            "subagent_stop",
        ] {
            assert_eq!(planner_activity(other, "{}"), None, "{other}");
        }
    }

    #[test]
    fn an_unknown_hook_name_is_refused() {
        let e = run("finished").expect_err("not a hook");
        assert!(e.to_string().contains("unknown hook"), "{e}");
    }
}
