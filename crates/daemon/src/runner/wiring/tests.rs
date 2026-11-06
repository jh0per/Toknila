use std::path::PathBuf;

use serde_json::Value;
use toknila_app::runner::RunBrief;
use toknila_core::hook::HOOKS;
use toknila_core::{AgentProfile, AgentRole, RunId};

use super::*;

const TOKEN: &str = "secret-run-token-0123456789abcdef";

struct Rig {
    dir: tempfile::TempDir,
    bin: PathBuf,
    key: TicketKey,
    run: RunId,
    brief: RunBrief,
}

impl Rig {
    fn new() -> Rig {
        Rig {
            dir: tempfile::tempdir().expect("tempdir"),
            bin: PathBuf::from("/opt/toknila/bin/toknila"),
            key: "DEMO-1".parse().expect("key"),
            run: RunId::new(),
            brief: RunBrief {
                title: "Wire it".to_owned(),
                prompt: "Wire the instructions.".to_owned(),
                acceptance: vec!["It is wired".to_owned()],
            },
        }
    }

    fn prepare(&self, profile: &AgentProfile) -> Wiring {
        self.prepare_as(profile, AgentRole::Worker)
    }

    fn prepare_as(&self, profile: &AgentProfile, role: AgentRole) -> Wiring {
        let cx = Context {
            dir: &self.dir.path().join("runs").join("t1"),
            key: &self.key,
            role,
            run: self.run,
            token: TOKEN,
            url: Some("http://127.0.0.1:4242"),
            bin: &self.bin,
            brief: &self.brief,
        };
        prepare(profile, &cx).expect("prepare")
    }

    fn file(&self, name: &str) -> (PathBuf, String) {
        let path = self.dir.path().join("runs").join("t1").join(name);
        let text = std::fs::read_to_string(&path).expect("generated file");
        (path, text)
    }
}

#[test]
fn the_built_in_profile_gets_both_files_and_the_flags_that_point_at_them() {
    let rig = Rig::new();
    let w = rig.prepare(&AgentProfile::claude_code());
    let (mcp_path, _) = rig.file("mcp.json");
    let (settings_path, _) = rig.file("settings.json");
    let (instructions_path, _) = rig.file("instructions.md");
    let want = [
        "--mcp-config".to_owned(),
        mcp_path.display().to_string(),
        "--settings".to_owned(),
        settings_path.display().to_string(),
        "--append-system-prompt-file".to_owned(),
        instructions_path.display().to_string(),
    ];
    assert_eq!(w.args, want);
}

#[test]
fn the_instructions_file_holds_the_task_and_lives_outside_the_worktree() {
    // The agent runs in a worktree that has its own CLAUDE.md; wiring never touches it and the
    // generated file sits under the data home.
    let rig = Rig::new();
    let ship = rig.dir.path().join("ship");
    std::fs::create_dir_all(&ship).expect("ship");
    std::fs::write(ship.join("CLAUDE.md"), "project rules").expect("claude.md");
    rig.prepare(&AgentProfile::claude_code());
    let (path, text) = rig.file("instructions.md");
    assert!(text.contains("# Toknila: ticket DEMO-1, Wire it"));
    assert!(text.contains("Wire the instructions."));
    assert!(text.contains("- [ ] It is wired"));
    assert!(!path.starts_with(&ship));
    assert_eq!(
        std::fs::read_to_string(ship.join("CLAUDE.md")).expect("read"),
        "project rules"
    );
    assert_eq!(std::fs::read_dir(&ship).expect("ls").count(), 1);
}

#[test]
fn a_profile_without_an_instructions_flag_gets_no_instructions_file() {
    let rig = Rig::new();
    let mut profile = AgentProfile::claude_code();
    profile.instructions_flag = None;
    let w = rig.prepare(&profile);
    assert!(!w.args.iter().any(|a| a.ends_with("instructions.md")));
    assert!(
        !rig.dir
            .path()
            .join("runs")
            .join("t1")
            .join("instructions.md")
            .exists()
    );
}

#[test]
fn the_mcp_config_runs_tok_mcp_and_names_variables_not_the_token() {
    let rig = Rig::new();
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("mcp.json");
    assert!(!text.contains(TOKEN), "the token is never written to disk");
    let v: Value = serde_json::from_str(&text).expect("json");
    let server = &v["mcpServers"]["toknila"];
    assert_eq!(server["command"], "/opt/toknila/bin/toknila");
    assert_eq!(server["args"], serde_json::json!(["mcp"]));
    assert_eq!(server["env"][agent_env::TOKEN], "${TOKNILA_RUN_TOKEN}");
    assert_eq!(server["env"][agent_env::URL], "${TOKNILA_URL}");
    assert_eq!(server["env"][agent_env::TICKET], "${TOKNILA_TICKET}");
    assert_eq!(server["env"][agent_env::ROLE], "${TOKNILA_ROLE}");
}

#[test]
fn the_hook_settings_run_tok_hook_for_each_subscribed_hook_and_never_finish() {
    let rig = Rig::new();
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("settings.json");
    assert!(!text.contains(TOKEN), "the token is never written to disk");
    let v: Value = serde_json::from_str(&text).expect("json");
    for (claude, ours) in HOOKS {
        let command = v["hooks"][claude][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_else(|| panic!("no hook {claude}: {text}"));
        assert_eq!(command, format!("\"/opt/toknila/bin/toknila\" hook {ours}"));
    }
    assert!(!text.contains("finish"));
}

#[test]
fn the_environment_carries_the_daemon_address_the_token_the_ticket_and_the_run() {
    let rig = Rig::new();
    let w = rig.prepare(&AgentProfile::claude_code());
    let env = |name: &str| {
        w.env
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(env("TOKNILA_URL"), Some("http://127.0.0.1:4242"));
    assert_eq!(env("TOKNILA_RUN_TOKEN"), Some(TOKEN));
    assert_eq!(env("TOKNILA_TICKET"), Some("DEMO-1"));
    assert_eq!(env("TOKNILA_RUN"), Some(rig.run.to_string().as_str()));
    assert_eq!(env("TOKNILA_ROLE"), Some("worker"));
    let reviewer = rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let role = reviewer.env.iter().find(|(k, _)| k == "TOKNILA_ROLE");
    assert_eq!(role.map(|(_, v)| v.as_str()), Some("reviewer"));
}

#[test]
fn a_profile_without_wiring_gets_no_files_and_no_flags() {
    let rig = Rig::new();
    let mut profile = AgentProfile::claude_code();
    profile.mcp.enabled = false;
    profile.hooks.enabled = false;
    profile.instructions_flag = None;
    let w = rig.prepare(&profile);
    assert!(w.args.is_empty());
    assert!(!rig.dir.path().join("runs").join("t1").exists());

    // Enabled but with no flag to pass the file by: nothing is written either.
    profile.mcp.enabled = true;
    profile.mcp.config_flag = None;
    assert!(rig.prepare(&profile).args.is_empty());
}
