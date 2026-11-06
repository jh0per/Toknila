//! Claude Code hooks as a second source of agent signals. Every hook that fires is a sign of
//! life (activity). Only the few that clearly mean something also map to an [`AgentEvent`]:
//! a permission prompt is `ask_human`. A `Stop` hook fires after every reply, so it is never
//! `finished`; the agent hands in through the event contract and nothing else does.
//!
//! Pure: the CLI reads the hook's JSON from stdin and the daemon records what comes of it.

use serde::Deserialize;

use crate::agent::AgentEvent;

/// The hooks Toknila subscribes to: Claude Code's name, and the name `tok hook <name>` takes.
pub const HOOKS: [(&str, &str); 5] = [
    ("Notification", "notification"),
    ("Stop", "stop"),
    ("SubagentStop", "subagent_stop"),
    ("UserPromptSubmit", "user_prompt_submit"),
    ("PostToolUse", "post_tool_use"),
];

/// The fields of a hook's JSON input that Toknila reads; everything else is ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HookInput {
    /// What a `Notification` hook is about, for example `permission_prompt` or `idle_prompt`.
    pub notification_type: Option<String>,
    /// The notification's text.
    pub message: Option<String>,
}

/// Whether `name` is a hook Toknila subscribes to.
pub fn is_known(name: &str) -> bool {
    HOOKS.iter().any(|(_, ours)| *ours == name)
}

/// The agent event a hook maps to, if it clearly means one. Every hook is activity either way.
pub fn event_for(name: &str, input: &HookInput) -> Option<AgentEvent> {
    match (name, input.notification_type.as_deref()) {
        ("notification", Some("permission_prompt")) => Some(AgentEvent::AskHuman {
            text: input
                .message
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "Claude Code asks for permission".to_owned()),
            permission: true,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: Option<&str>, message: Option<&str>) -> HookInput {
        HookInput {
            notification_type: kind.map(str::to_owned),
            message: message.map(str::to_owned),
        }
    }

    #[test]
    fn a_permission_prompt_asks_a_human_for_permission() {
        let i = input(
            Some("permission_prompt"),
            Some("Claude needs your permission to use Bash"),
        );
        assert_eq!(
            event_for("notification", &i),
            Some(AgentEvent::AskHuman {
                text: "Claude needs your permission to use Bash".to_owned(),
                permission: true,
            })
        );
    }

    #[test]
    fn a_permission_prompt_without_text_still_says_what_it_is() {
        let i = input(Some("permission_prompt"), None);
        let Some(AgentEvent::AskHuman { text, permission }) = event_for("notification", &i) else {
            panic!("expected ask_human");
        };
        assert!(permission);
        assert!(!text.is_empty());
    }

    #[test]
    fn other_notifications_are_only_activity() {
        for kind in [
            Some("idle_prompt"),
            Some("auth_success"),
            Some("whatever"),
            None,
        ] {
            let i = input(kind, Some("hi"));
            assert_eq!(event_for("notification", &i), None, "{kind:?}");
        }
    }

    #[test]
    fn stop_is_never_finished() {
        let i = input(Some("permission_prompt"), Some("x"));
        for name in [
            "stop",
            "subagent_stop",
            "user_prompt_submit",
            "post_tool_use",
        ] {
            assert_eq!(event_for(name, &i), None, "{name}");
        }
    }

    #[test]
    fn every_subscribed_hook_is_known_and_unknown_names_are_not() {
        for (_, ours) in HOOKS {
            assert!(is_known(ours), "{ours}");
        }
        assert!(!is_known("Stop"));
        assert!(!is_known("finished"));
    }

    #[test]
    fn hook_json_is_read_leniently() {
        let i: HookInput = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","notification_type":"permission_prompt","message":"m"}"#,
        )
        .expect("parse");
        assert_eq!(i, input(Some("permission_prompt"), Some("m")));
        let empty: HookInput = serde_json::from_str("{}").expect("parse");
        assert_eq!(empty, HookInput::default());
    }
}
