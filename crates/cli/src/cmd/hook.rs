//! `tok hook <name>`: a Claude Code hook as a second source of agent signals. The hook's JSON
//! arrives on stdin. Every hook is recorded as activity; the few that clearly mean an event
//! (a permission prompt) are also sent as one. Which is core's rule (`toknila_core::hook`).
//!
//! A hook must never get in the agent's way: whatever goes wrong here is a line on stderr and
//! exit code 0 (Claude Code treats exit code 2 as "block this action").

use std::io::Read;

use anyhow::{Result, bail};
use toknila_api_types::{AgentEvent, InstructionClaim, NewActivity};
use toknila_core::hook::{self, HookInput};

use super::agent::AgentEnv;

/// What a hook says, from its name and its stdin: the activity signal (with the session it fired
/// in, for the usage), and the event if any.
fn signals(name: &str, stdin: &str) -> (NewActivity, Option<AgentEvent>) {
    // A hook payload this version does not understand is still a sign of life.
    let input: HookInput = serde_json::from_str(stdin).unwrap_or_default();
    let activity = NewActivity {
        signal: format!("hook:{name}"),
        session_id: input.session_id.clone(),
        transcript_path: input.transcript_path.clone(),
    };
    (activity, hook::event_for(name, &input))
}

/// What a Stop hook prints to keep the agent working: the instructions a person left, as Claude
/// Code's block decision. Nothing when none were pending, so the agent stops. That also ends the
/// loop when the stop was itself caused by this hook (`stop_hook_active`): a claim hands each
/// instruction out once, so the next stop finds nothing.
fn stop_output(claim: &InstructionClaim) -> Option<String> {
    (claim.count > 0)
        .then(|| serde_json::json!({ "decision": "block", "reason": claim.reason }).to_string())
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
    let (activity, event) = signals(name, &stdin);
    let sent = agent
        .client
        .activity(&agent.ticket, &activity)
        .map_err(|e| e.to_string())
        .and_then(|()| match event {
            Some(e) => agent
                .client
                .agent_event(&agent.ticket, &e)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            None => Ok(()),
        });
    if let Err(e) = sent {
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
    fn a_permission_prompt_is_activity_and_ask_human() {
        let json = r#"{"hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow Bash?"}"#;
        let (activity, event) = signals("notification", json);
        assert_eq!(activity.signal, "hook:notification");
        assert_eq!(
            event,
            Some(AgentEvent::AskHuman {
                text: "Allow Bash?".to_owned(),
                permission: true
            })
        );
    }

    #[test]
    fn stop_and_garbage_are_activity_only() {
        let only = |signal: &str| NewActivity {
            signal: signal.to_owned(),
            session_id: None,
            transcript_path: None,
        };
        let stop = r#"{"hook_event_name":"Stop"}"#;
        assert_eq!(signals("stop", stop), (only("hook:stop"), None));
        assert_eq!(
            signals("notification", "not json"),
            (only("hook:notification"), None)
        );
        assert_eq!(signals("stop", ""), (only("hook:stop"), None));
    }

    #[test]
    fn the_session_and_its_transcript_travel_with_the_activity() {
        let json = r#"{"hook_event_name":"Stop","session_id":"abc","transcript_path":"/h/.claude/projects/p/abc.jsonl"}"#;
        let (activity, event) = signals("stop", json);
        assert_eq!(activity.session_id.as_deref(), Some("abc"));
        assert_eq!(
            activity.transcript_path.as_deref(),
            Some("/h/.claude/projects/p/abc.jsonl")
        );
        assert_eq!(event, None);
    }

    #[test]
    fn an_unknown_hook_name_is_refused() {
        let e = run("finished").expect_err("not a hook");
        assert!(e.to_string().contains("unknown hook"), "{e}");
    }
}
