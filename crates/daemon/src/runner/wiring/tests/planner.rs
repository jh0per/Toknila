//! What a project's planner run is given: no ticket anywhere, its own role and files, the
//! planner's instructions, and rules that let it read and use its tools but not write.

use serde_json::Value;
use toknila_core::instructions::ProjectFacts;
use toknila_core::{AgentProfile, AgentRole, MergeMethod, MergeMode, TicketKey};

use super::{Rig, TOKEN};

fn planner_rig() -> Rig {
    let mut rig = Rig::new();
    rig.key = TicketKey::planner("DEMO".parse().expect("key"));
    rig.brief.title = "Demo".to_owned();
    rig.brief.check_command = Some("cargo xtask verify".to_owned());
    rig.brief.project = Some(ProjectFacts {
        key: "DEMO".to_owned(),
        name: "Demo".to_owned(),
        work_branch: "develop".to_owned(),
        merge_mode: MergeMode::Github,
        merge_method: MergeMethod::Squash,
        check_command: Some("cargo xtask verify".to_owned()),
        base_behind: None,
    });
    rig
}

#[test]
fn the_planner_gets_the_role_the_token_and_no_ticket() {
    let rig = planner_rig();
    let w = rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let env = |name: &str| {
        w.env
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(env("TOKNILA_ROLE"), Some("planner"));
    assert_eq!(env("TOKNILA_RUN_TOKEN"), Some(TOKEN));
    assert_eq!(env("TOKNILA_TICKET"), None, "a planner has no card");
    assert_eq!(env("CLAUDE_CODE_DISABLE_AUTO_MEMORY"), Some("1"));
    // The first prompt names the project, not a card.
    let kickoff = w.command.last().expect("kickoff");
    assert!(kickoff.contains("planner of project DEMO"), "{kickoff}");
    assert!(!kickoff.contains("DEMO-0"));
}

#[test]
fn the_mcp_config_of_a_planner_names_no_ticket_variable() {
    let rig = planner_rig();
    rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let (_, text) = rig.file_of("planner", "mcp.json");
    assert!(!text.contains(TOKEN));
    let v: Value = serde_json::from_str(&text).expect("json");
    let env = &v["mcpServers"]["toknila"]["env"];
    assert_eq!(env["TOKNILA_ROLE"], "${TOKNILA_ROLE}");
    assert_eq!(env["TOKNILA_RUN_TOKEN"], "${TOKNILA_RUN_TOKEN}");
    assert!(
        env.get("TOKNILA_TICKET").is_none(),
        "a reference to an unset variable would stop the config from loading: {env}"
    );
}

#[test]
fn the_planner_reads_its_instructions_from_its_own_folder() {
    let rig = planner_rig();
    let w = rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let (path, text) = rig.file_of("planner", "instructions.md");
    assert!(text.contains("# Toknila: planner of DEMO, Demo"));
    assert!(text.contains("pull request against `develop`"));
    assert!(text.contains("`cargo xtask verify`"));
    assert!(text.contains("`show_plan`"));
    assert!(w.args.contains(&path.display().to_string()));
    // The worker's and the reviewer's folders are untouched.
    assert!(!path.to_string_lossy().contains("worker"));
}

#[test]
fn the_planner_settings_allow_reading_and_its_tools_and_name_no_hooks() {
    let rig = planner_rig();
    rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let (_, text) = rig.file_of("planner", "settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    assert!(
        v.get("hooks").is_none(),
        "a planner is not a card's agent: {text}"
    );
    assert!(v.get("statusLine").is_none());
    // A planner is a normal role: it gets what every role gets (auto memory off, no subagents, no tool that leaves the ship).
    assert_eq!(v["autoMemoryEnabled"], false);
    assert_eq!(
        v["permissions"]["deny"],
        serde_json::json!(super::denied_with_private())
    );
    let allow: Vec<&str> = v["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for want in ["Read", "Grep", "Glob", "Bash(git status *)", "mcp__toknila"] {
        assert!(allow.contains(&want), "{want}: {allow:?}");
    }
    for never in [
        "Edit",
        "Write",
        "NotebookEdit",
        "Bash(git commit *)",
        "Bash(git add *)",
    ] {
        assert!(
            !allow.contains(&never),
            "{never} must not be allowed: {allow:?}"
        );
    }
    assert!(
        !allow.iter().any(|r| r.contains("cargo xtask")),
        "the planner does not run the project's checks: {allow:?}"
    );
}

#[test]
fn a_planner_run_without_the_projects_facts_is_refused() {
    let mut rig = planner_rig();
    rig.brief.project = None;
    let cx = super::Context {
        dir: &rig.dir.path().join("runs").join("t1"),
        key: &rig.key,
        role: AgentRole::Planner,
        run: rig.run,
        token: TOKEN,
        url: None,
        bin: &rig.bin,
        brief: &rig.brief,
    };
    let e = super::prepare(&AgentProfile::claude_code_planner(), &cx).expect_err("no facts");
    assert!(e.to_string().contains("project's facts"), "{e}");
}
