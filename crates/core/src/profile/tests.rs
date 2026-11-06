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
    assert_eq!((p.nudge.idle_secs, p.nudge.max_nudges), (60, 1));
    assert!(p.output.strip_ansi, "unset sections take their defaults");
}

#[test]
fn only_the_command_is_required() {
    let p = parse("min", r#"command = ["agent"]"#).expect("valid");
    assert_eq!(p.role, AgentRole::Worker);
    assert_eq!(p.nudge, crate::Nudge::default());
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
    assert_eq!(c.profiles[1].profile.command, ["claude"], "no override");
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
    assert_eq!(AgentProfile::claude_code().command, ["claude"]);
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
