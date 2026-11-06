use super::*;
use crate::{AgentProfile, AgentRole};

const FULL: &str = r#"role = "reviewer"
command = ["my-agent", "--headless"]
instructions_file = "AGENTS.md"
instructions_flag = "--append-system-prompt-file"

[env]
MY_AGENT_MODE = "headless"

[mcp]
enabled = true
config_flag = "--mcp-config"

[nudge]
idle_secs = 60
max_nudges = 1
window_secs = 600
message = "Report, please."
"#;

fn err(text: &str) -> ProfileError {
    parse("x", text).expect_err("the file is invalid")
}

fn file(name: &str, text: &str) -> ProfileFile {
    ProfileFile {
        file: name.to_owned(),
        text: Ok(text.to_owned()),
    }
}

#[test]
fn a_profile_file_becomes_a_profile_named_after_the_file() {
    let p = parse("my-agent", FULL).expect("valid");
    assert_eq!(p.name, "my-agent");
    assert_eq!(p.role, AgentRole::Reviewer);
    assert_eq!(p.command, ["my-agent", "--headless"]);
    assert_eq!(p.env["MY_AGENT_MODE"], "headless");
    assert_eq!(p.instructions_file.as_deref(), Some("AGENTS.md"));
    assert_eq!(
        p.instructions_flag.as_deref(),
        Some("--append-system-prompt-file")
    );
    assert!(p.mcp.enabled && !p.hooks.enabled);
    assert_eq!(
        (p.nudge.idle_secs, p.nudge.max_nudges, p.nudge.window_secs),
        (60, 1, 600)
    );
    assert!(p.output.strip_ansi, "unset sections take their defaults");
}

#[test]
fn only_the_command_is_required() {
    let p = parse("min", r#"command = ["agent"]"#).expect("valid");
    assert_eq!(p.role, AgentRole::Worker);
    assert_eq!(p.nudge, crate::Nudge::default());
    let n = &p.nudge;
    assert_eq!((n.idle_secs, n.max_nudges, n.window_secs), (120, 3, 900));
}

#[test]
fn an_unknown_field_names_its_line_and_column() {
    let e = err("command = [\"a\"]\n\n[nudge]\nidle_sec = 5\n");
    assert_eq!((e.line, e.column), (4, 1), "{e}");
    assert!(e.message.contains("idle_sec"), "{e}");
    let e = err("command = [\"a\"]\ncomand = 1\n");
    assert_eq!((e.line, e.column), (2, 1), "{e}");
}

#[test]
fn a_wrong_type_names_the_value() {
    let e = err("command = \"claude --flag\"\n");
    assert_eq!(
        (e.line, e.column),
        (1, 11),
        "a shell string is not argv: {e}"
    );
    let e = err("command = [\"a\"]\nrole = \"boss\"\n");
    assert_eq!((e.line, e.column), (2, 8), "{e}");
    let e = err("command = [\"a\"]\n[nudge]\nidle_secs = \"soon\"\n");
    assert_eq!((e.line, e.column), (3, 13), "{e}");
}

#[test]
fn a_missing_command_is_reported() {
    let e = err("role = \"worker\"\n");
    assert!(e.message.contains("command"), "{e}");
    assert_eq!(e.line, 1);
}

#[test]
fn an_empty_command_or_program_is_reported_at_the_command() {
    for text in ["command = []\n", "command = [\"\"]\n"] {
        let e = err(&format!("role = \"worker\"\n{text}"));
        assert_eq!((e.line, e.column), (2, 11), "{e}");
        assert!(e.message.contains("program"), "{e}");
    }
}

#[test]
fn bad_env_names_and_a_zero_idle_time_are_reported() {
    let e = err("command = [\"a\"]\n[env]\n\"A=B\" = \"x\"\n");
    assert_eq!(e.line, 3, "{e}");
    let e = err("command = [\"a\"]\n[nudge]\nidle_secs = 0\n");
    assert_eq!(e.line, 3, "{e}");
}

#[test]
fn a_file_name_that_is_not_a_profile_name_is_rejected() {
    for name in ["", "Claude", "my agent", "-x"] {
        assert!(parse(name, "command = [\"a\"]").is_err(), "{name:?}");
    }
}

#[test]
fn the_catalog_lists_built_ins_then_files_by_name_and_skips_other_files() {
    let c = catalog(vec![
        file("zeta.toml", "command = [\"z\"]"),
        file("notes.txt", "not a profile"),
        file("alpha.toml", "command = [\"a\"]"),
    ]);
    let names: Vec<(&str, Option<&str>)> = c
        .profiles
        .iter()
        .map(|l| (l.profile.name.as_str(), l.file.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("sim", None),
            ("claude-code", None),
            ("sim-reviewer", None),
            ("claude-code-reviewer", None),
            ("alpha", Some("alpha.toml")),
            ("zeta", Some("zeta.toml")),
        ]
    );
    assert!(c.problems.is_empty());
}

#[test]
fn the_catalog_reports_bad_files_and_still_lists_the_good_ones() {
    let c = catalog(vec![
        file("bad.toml", "command = [\"a\"]\nflavour = 1\n"),
        file("claude-code.toml", "command = [\"mine\"]"),
        ProfileFile {
            file: "gone.toml".to_owned(),
            text: Err("permission denied".to_owned()),
        },
        file("good.toml", "command = [\"g\"]"),
    ]);
    assert_eq!(c.profiles.len(), 5, "four built-ins and good");
    assert_eq!(
        c.profiles[1].profile.command,
        ["claude", "--permission-mode", "acceptEdits", "{kickoff}"],
        "no override"
    );
    let got: Vec<(&str, u32, u32)> = c
        .problems
        .iter()
        .map(|p| (p.file.as_str(), p.line, p.column))
        .collect();
    assert_eq!(
        got,
        [
            ("bad.toml", 2, 1),
            ("claude-code.toml", 1, 1),
            ("gone.toml", 1, 1)
        ]
    );
    assert!(c.problems[1].message.contains("built-in"));
    assert!(c.problems[2].message.contains("permission denied"));
}

#[test]
fn the_built_in_profiles_launch_what_they_say() {
    assert!(
        AgentProfile::sim().command.is_empty(),
        "sim starts no process"
    );
    assert_eq!(
        AgentProfile::claude_code().command,
        ["claude", "--permission-mode", "acceptEdits", "{kickoff}"]
    );
    let claude = AgentProfile::claude_code();
    assert!(claude.mcp.enabled && claude.hooks.enabled);
    assert_eq!(claude.mcp.config_flag.as_deref(), Some("--mcp-config"));
    assert_eq!(claude.hooks.settings_flag.as_deref(), Some("--settings"));
    assert_eq!(
        claude.instructions_flag.as_deref(),
        Some("--append-system-prompt-file")
    );
}

/// Profile JSON is read by the CLI, the web page and the launcher; changes must be deliberate.
/// `TOKNILA_UPDATE_SNAPSHOTS=1` rewrites the fixture.
#[test]
fn profile_json_is_pinned() {
    let c = catalog(vec![file("my-agent.toml", FULL)]);
    let text = format!("{}\n", serde_json::to_string_pretty(&c).expect("json"));
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/profile/snapshots/catalog.json");
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &text).expect("write snapshot");
        return;
    }
    let want = std::fs::read_to_string(&path)
        .expect("missing snapshot; run with TOKNILA_UPDATE_SNAPSHOTS=1")
        .replace("\r\n", "\n");
    assert_eq!(
        text, want,
        "profile JSON changed; if intended, rerun with TOKNILA_UPDATE_SNAPSHOTS=1"
    );
}

#[test]
fn the_kickoff_placeholder_is_filled_wherever_it_stands_in_the_command() {
    let p = parse(
        "x",
        r#"command = ["agent", "--prompt={kickoff}", "{kickoff}", "--other"]"#,
    )
    .expect("valid");
    assert_eq!(
        p.argv("begin\nnow"),
        ["agent", "--prompt=begin\nnow", "begin\nnow", "--other"]
    );
    let none = parse("y", r#"command = ["agent"]"#).expect("valid");
    assert_eq!(
        none.argv("begin"),
        ["agent"],
        "no placeholder, no first prompt"
    );
}

#[test]
fn the_built_in_claude_profiles_pass_the_kickoff_and_the_reviewer_cannot_ask() {
    let worker = AgentProfile::claude_code();
    let reviewer = AgentProfile::claude_code_reviewer();
    for p in [&worker, &reviewer] {
        assert!(
            p.command.iter().any(|a| a == AgentProfile::KICKOFF),
            "{}",
            p.name
        );
    }
    assert_eq!(
        reviewer.command,
        ["claude", "--permission-mode", "dontAsk", "{kickoff}"]
    );
    assert!(worker.command.iter().any(|a| a == "acceptEdits"));
    assert!(!worker.command.iter().any(|a| a == "dontAsk"));
}

#[test]
fn the_reviewers_allow_list_covers_read_only_git_the_check_and_the_filters_of_a_pipe() {
    let rules = AgentProfile::claude_code_reviewer()
        .permissions
        .rules(Some("cargo xtask verify"));
    for want in [
        "Bash(git diff *)",
        "Bash(git log *)",
        "Bash(git show *)",
        "Bash(git status *)",
        "Bash(cargo xtask verify *)",
        "Bash(grep *)",
        "Bash(head *)",
        "Bash(tail *)",
        "Bash(wc *)",
        "mcp__toknila",
    ] {
        assert!(rules.iter().any(|r| r == want), "{want} in {rules:?}");
    }
    for never in ["git push", "git commit", "git add", "rm "] {
        assert!(
            !rules.iter().any(|r| r.contains(never)),
            "{never} in {rules:?}"
        );
    }
}

#[test]
fn a_rule_that_needs_the_check_command_is_dropped_without_one() {
    let p = AgentProfile::claude_code();
    let with = p.permissions.rules(Some("make check"));
    let without = p.permissions.rules(None);
    assert!(with.iter().any(|r| r == "Bash(make check *)"));
    assert!(without.iter().all(|r| !r.contains("check")), "{without:?}");
    assert_eq!(with.len(), without.len() + 1);
}

#[test]
fn permissions_are_read_from_the_profile_file() {
    let p = parse(
        "x",
        "command = [\"a\"]\n[permissions]\nallow = [\"Bash(ls *)\"]\n",
    )
    .expect("valid");
    assert_eq!(p.permissions.allow, ["Bash(ls *)"]);
    let e = err("command = [\"a\"]\n[permissions]\ndeny = []\n");
    assert_eq!(e.line, 3, "{e}");
}

#[test]
fn claude_code_variables_are_dropped_unless_the_profile_sets_them() {
    let mut p = AgentProfile::claude_code();
    p.env.insert("CLAUDE_CODE_USE_BEDROCK".into(), "1".into());
    let names = [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_USE_BEDROCK",
        "claude_code_child_session",
        "CLAUDE_CONFIG_DIR",
        "PATH",
        "ANTHROPIC_API_KEY",
    ]
    .map(str::to_owned);
    assert_eq!(
        p.inherited_to_drop(names),
        [
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "claude_code_child_session"
        ]
    );
}
