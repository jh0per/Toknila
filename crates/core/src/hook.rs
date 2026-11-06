//! Claude Code hooks as a second source of agent signals. Every hook that fires is a sign of
//! life (activity). Only a permission prompt also means something: the person is asked (the
//! daemon turns it into `ask_human` for a worker, or an inbox item for a reviewer). A `Stop` hook
//! fires after every reply, so it is never `finished`; the agent hands in through the event
//! contract and nothing else does.
//!
//! A `PreToolUse` hook tells which tool is about to run; the summary of it (the command, or the
//! file) is what a later permission prompt is about. A `PermissionRequest` hook fires when the
//! agent is about to ask the person and may answer for them; Toknila holds it until the person
//! answers in the window.
//!
//! Pure: the CLI reads the hook's JSON from stdin and the daemon records what comes of it.

use serde::Deserialize;

/// The hooks Toknila subscribes to: Claude Code's name, and the name `tok hook <name>` takes.
pub const HOOKS: [(&str, &str); 7] = [
    ("Notification", "notification"),
    ("Stop", "stop"),
    ("SubagentStop", "subagent_stop"),
    ("UserPromptSubmit", "user_prompt_submit"),
    ("PostToolUse", "post_tool_use"),
    ("PreToolUse", "pre_tool_use"),
    ("PermissionRequest", "permission_request"),
];

/// The timeout, in seconds, written into the settings of the permission hook: how long Claude
/// Code lets it wait for the person.
pub const PERMISSION_HOOK_TIMEOUT_SECS: u64 = 600;

/// How long the daemon holds the permission hook for an answer before it lets the agent show
/// its own dialog; shorter than the hook's timeout so the reply always arrives in time.
pub const PERMISSION_WAIT_SECS: u64 = 570;

/// The longest summary of a tool call that is kept, in characters.
pub const SUMMARY_MAX: usize = 300;

/// The fields of a hook's JSON input that Toknila reads; everything else is ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HookInput {
    /// What a `Notification` hook is about, for example `permission_prompt` or `idle_prompt`.
    pub notification_type: Option<String>,
    /// The notification's text.
    pub message: Option<String>,
    /// Claude Code's id of the session the hook fired in.
    pub session_id: Option<String>,
    /// The session's transcript file.
    pub transcript_path: Option<String>,
    /// The tool a `PreToolUse` or `PermissionRequest` hook is about (`Bash`, `Write`, ...).
    pub tool_name: Option<String>,
    /// The tool's arguments; only the command or the path is read from them.
    pub tool_input: Option<serde_json::Value>,
}

/// A short, single-line account of a tool call: `Bash: cargo test`, `Write: src/lib.rs`. Only the
/// command, the file path, the pattern or the URL is kept, never the contents of a file or a
/// prompt, and the whole is cut at [`SUMMARY_MAX`] characters.
pub fn tool_summary(input: &HookInput) -> Option<String> {
    let name = input
        .tool_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())?;
    let arg = |key: &str| {
        input
            .tool_input
            .as_ref()
            .and_then(|v| v.get(key))
            .and_then(|v| v.as_str())
    };
    let detail = match name {
        "Bash" => arg("command"),
        "Write" | "Edit" | "MultiEdit" | "Read" | "NotebookEdit" => {
            arg("file_path").or_else(|| arg("notebook_path"))
        }
        "Glob" | "Grep" => arg("pattern"),
        "WebFetch" => arg("url"),
        _ => None,
    };
    let text = match detail {
        Some(d) => format!("{name}: {d}"),
        None => name.to_owned(),
    };
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(cut(&one_line, SUMMARY_MAX))
}

/// A tool summary some client sent, brought to a single line of at most [`SUMMARY_MAX`] characters.
pub fn cap_summary(text: &str) -> String {
    cut(
        &text.split_whitespace().collect::<Vec<_>>().join(" "),
        SUMMARY_MAX,
    )
}

/// `text` cut to `max` characters, with an ellipsis when something was dropped.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// The question a permission prompt makes: the notification's own text, and what the agent last
/// tried to run when that is known.
pub fn permission_text(message: Option<&str>, tool: Option<&str>) -> String {
    let message = message
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or("Claude Code asks for permission");
    match tool.map(str::trim).filter(|t| !t.is_empty()) {
        Some(tool) => format!("{message}: {tool}"),
        None => message.to_owned(),
    }
}

/// Whether an activity signal shows the agent got past a prompt (it ran a tool, took a prompt or
/// stopped), so a question that was open for it is gone.
pub fn clears_prompt(signal: &str) -> bool {
    matches!(
        signal,
        "hook:post_tool_use" | "hook:user_prompt_submit" | "hook:stop" | "hook:subagent_stop"
    )
}

/// Whether `name` is a hook Toknila subscribes to.
pub fn is_known(name: &str) -> bool {
    HOOKS.iter().any(|(_, ours)| *ours == name)
}

/// The message of the permission prompt a hook reports, if it is one (a `Notification` of type
/// `permission_prompt`; empty when it carries none). Every hook is activity either way.
pub fn permission_prompt(name: &str, input: &HookInput) -> Option<String> {
    match (name, input.notification_type.as_deref()) {
        ("notification", Some("permission_prompt")) => {
            Some(input.message.clone().unwrap_or_default())
        }
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
            ..HookInput::default()
        }
    }

    #[test]
    fn a_permission_prompt_notification_carries_its_message() {
        let i = input(
            Some("permission_prompt"),
            Some("Claude needs your permission to use Bash"),
        );
        assert_eq!(
            permission_prompt("notification", &i).as_deref(),
            Some("Claude needs your permission to use Bash")
        );
        let none = input(Some("permission_prompt"), None);
        assert_eq!(
            permission_prompt("notification", &none).as_deref(),
            Some("")
        );
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
            assert_eq!(permission_prompt("notification", &i), None, "{kind:?}");
        }
    }

    #[test]
    fn no_other_hook_is_a_permission_prompt() {
        let i = input(Some("permission_prompt"), Some("x"));
        for name in [
            "stop",
            "subagent_stop",
            "user_prompt_submit",
            "post_tool_use",
            "pre_tool_use",
            "permission_request",
        ] {
            assert_eq!(permission_prompt(name, &i), None, "{name}");
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

    fn tool(name: &str, input: serde_json::Value) -> HookInput {
        HookInput {
            tool_name: Some(name.to_owned()),
            tool_input: Some(input),
            ..HookInput::default()
        }
    }

    #[test]
    fn a_tool_summary_is_the_command_or_the_path_and_nothing_else() {
        use serde_json::json;
        let cases = [
            (
                tool(
                    "Bash",
                    json!({"command": "cargo  test\n  -p core", "description": "d"}),
                ),
                "Bash: cargo test -p core",
            ),
            (
                tool(
                    "Write",
                    json!({"file_path": "/w/a.rs", "content": "SECRET"}),
                ),
                "Write: /w/a.rs",
            ),
            (
                tool(
                    "Edit",
                    json!({"file_path": "/w/b.rs", "old_string": "SECRET", "new_string": "x"}),
                ),
                "Edit: /w/b.rs",
            ),
            (
                tool("Grep", json!({"pattern": "TODO", "path": "/w"})),
                "Grep: TODO",
            ),
            (tool("mcp__x__y", json!({"prompt": "SECRET"})), "mcp__x__y"),
        ];
        for (input, want) in cases {
            let got = tool_summary(&input).expect("summary");
            assert_eq!(got, want);
            assert!(!got.contains("SECRET"));
        }
        assert_eq!(tool_summary(&HookInput::default()), None);
    }

    #[test]
    fn a_long_summary_is_cut_at_the_limit() {
        let long = "x".repeat(1000);
        let input = tool("Bash", serde_json::json!({ "command": long }));
        let got = tool_summary(&input).expect("summary");
        assert_eq!(got.chars().count(), SUMMARY_MAX);
        assert!(got.ends_with('…'));
    }

    #[test]
    fn the_permission_text_names_the_tool_when_it_is_known() {
        assert_eq!(
            permission_text(Some("Claude needs your permission"), Some("Bash: ls")),
            "Claude needs your permission: Bash: ls"
        );
        assert_eq!(
            permission_text(Some(" "), None),
            "Claude Code asks for permission"
        );
    }

    #[test]
    fn only_progress_signals_clear_a_prompt() {
        for s in [
            "hook:post_tool_use",
            "hook:user_prompt_submit",
            "hook:stop",
            "hook:subagent_stop",
        ] {
            assert!(clears_prompt(s), "{s}");
        }
        for s in [
            "hook:notification",
            "hook:pre_tool_use",
            "hook:permission_request",
            "api",
            "start",
        ] {
            assert!(!clears_prompt(s), "{s}");
        }
    }

    #[test]
    fn hook_json_names_the_session_and_its_transcript() {
        let i: HookInput = serde_json::from_str(
            r#"{"hook_event_name":"Stop","session_id":"abc","transcript_path":"/h/.claude/projects/p/abc.jsonl"}"#,
        )
        .expect("parse");
        assert_eq!(i.session_id.as_deref(), Some("abc"));
        assert_eq!(
            i.transcript_path.as_deref(),
            Some("/h/.claude/projects/p/abc.jsonl")
        );
    }

    #[test]
    fn hook_json_is_read_leniently() {
        let i: HookInput = serde_json::from_str(
            r#"{"hook_event_name":"Notification","session_id":"s","notification_type":"permission_prompt","message":"m"}"#,
        )
        .expect("parse");
        assert_eq!(
            i,
            HookInput {
                session_id: Some("s".to_owned()),
                ..input(Some("permission_prompt"), Some("m"))
            }
        );
        let empty: HookInput = serde_json::from_str("{}").expect("parse");
        assert_eq!(empty, HookInput::default());
    }
}
