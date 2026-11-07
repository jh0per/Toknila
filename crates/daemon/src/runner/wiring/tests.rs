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
                check_command: None,
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
        self.file_of("worker", name)
    }

    fn file_of(&self, role: &str, name: &str) -> (PathBuf, String) {
        let path = self
            .dir
            .path()
            .join("runs")
            .join("t1")
            .join(role)
            .join(name);
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
            .join("worker")
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
fn the_settings_also_name_a_status_line_that_reports_the_plan_limits() {
    let rig = Rig::new();
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(v["statusLine"]["type"], "command");
    assert_eq!(
        v["statusLine"]["command"],
        "\"/opt/toknila/bin/toknila\" statusline"
    );
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
    profile.hooks.settings_flag = None;
    profile.instructions_flag = None;
    profile.permissions.allow.clear();
    let w = rig.prepare(&profile);
    assert!(w.args.is_empty());
    assert!(!w.env.iter().any(|(k, _)| k.contains("AUTO_MEMORY")));
    assert!(!rig.dir.path().join("runs").join("t1").exists());

    // Enabled but with no flag to pass the file by: nothing is written either.
    profile.mcp.enabled = true;
    profile.mcp.config_flag = None;
    assert!(rig.prepare(&profile).args.is_empty());
}

fn rules(text: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(text).expect("json");
    v["permissions"]["allow"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| r.as_str().expect("rule").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_worker_and_the_reviewer_of_a_ticket_get_files_of_their_own() {
    let rig = Rig::new();
    rig.prepare_as(&AgentProfile::claude_code(), AgentRole::Worker);
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    for name in ["instructions.md", "mcp.json", "settings.json"] {
        let (w, worker) = rig.file_of("worker", name);
        let (r, reviewer) = rig.file_of("reviewer", name);
        assert_ne!(w, r, "{name}");
        if name == "instructions.md" {
            assert!(worker.contains("# Toknila: ticket DEMO-1"), "{worker}");
            assert!(
                reviewer.contains("# Toknila: review of ticket DEMO-1"),
                "{reviewer}"
            );
        }
    }
    let flat = rig
        .dir
        .path()
        .join("runs")
        .join("t1")
        .join("instructions.md");
    assert!(!flat.exists(), "nothing is written in the old flat layout");
}

#[test]
fn the_first_prompt_fills_the_kickoff_placeholder_per_role() {
    let rig = Rig::new();
    let w = rig.prepare_as(&AgentProfile::claude_code(), AgentRole::Worker);
    assert_eq!(w.command[0], "claude");
    assert!(
        w.command[3].starts_with("Start ticket DEMO-1 now"),
        "{:?}",
        w.command
    );
    assert!(w.command[3].contains("`finished`"));
    let r = rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    assert_eq!(&r.command[..3], ["claude", "--permission-mode", "dontAsk"]);
    assert!(
        r.command[3].starts_with("Review ticket DEMO-1 now"),
        "{:?}",
        r.command
    );
    assert!(r.command[3].contains("`review_verdict`"));
    assert!(
        !r.command
            .iter()
            .chain(&w.command)
            .any(|a| a.contains("{kickoff}")),
        "the placeholder is always filled"
    );
}

#[test]
fn the_check_command_reaches_the_instructions_the_kickoff_and_both_allow_lists() {
    let mut rig = Rig::new();
    rig.brief.check_command = Some("cargo xtask verify".to_owned());
    let w = rig.prepare_as(&AgentProfile::claude_code(), AgentRole::Worker);
    assert!(
        w.command[3].contains("Run `cargo xtask verify`"),
        "{:?}",
        w.command
    );
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    for role in ["worker", "reviewer"] {
        let (_, instructions) = rig.file_of(role, "instructions.md");
        assert!(instructions.contains("`cargo xtask verify`"), "{role}");
        let (_, settings) = rig.file_of(role, "settings.json");
        assert!(
            rules(&settings).contains(&"Bash(cargo xtask verify *)".to_owned()),
            "{role}: {settings}"
        );
    }
}

#[test]
fn without_a_check_command_no_rule_names_one() {
    let rig = Rig::new();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, settings) = rig.file_of("reviewer", "settings.json");
    let allowed = rules(&settings);
    assert!(
        allowed.contains(&"Bash(git diff *)".to_owned()),
        "{allowed:?}"
    );
    assert!(
        allowed.iter().all(|r| !r.contains("{check_command}")),
        "{allowed:?}"
    );
    assert!(
        allowed.iter().all(|r| !r.starts_with("Bash( ")),
        "{allowed:?}"
    );
    let (_, instructions) = rig.file_of("reviewer", "instructions.md");
    assert!(!instructions.contains("## Checks"));
}

#[test]
fn the_settings_hold_hooks_and_rules_side_by_side() {
    let rig = Rig::new();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, text) = rig.file_of("reviewer", "settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    assert!(v["hooks"]["Stop"].is_array(), "{text}");
    assert!(v["permissions"]["allow"].is_array(), "{text}");
    assert!(
        v["permissions"].get("defaultMode").is_none(),
        "the mode is a flag, not a setting"
    );
}

#[test]
fn a_profile_without_rules_and_hooks_still_gets_the_isolation_settings() {
    let rig = Rig::new();
    let mut profile = AgentProfile::claude_code();
    profile.hooks.enabled = false;
    profile.permissions.allow.clear();
    let w = rig.prepare(&profile);
    assert!(w.args.iter().any(|a| a == "--settings"), "{:?}", w.args);
    let (_, text) = rig.file("settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(
        v,
        json!({ "autoMemoryEnabled": false, "permissions": { "deny": ["Agent"] } }),
        "{text}"
    );
}

#[test]
fn every_role_runs_without_auto_memory_and_without_the_subagent_tool() {
    let rig = Rig::new();
    for (role, dir, profile) in [
        (AgentRole::Worker, "worker", AgentProfile::claude_code()),
        (
            AgentRole::Reviewer,
            "reviewer",
            AgentProfile::claude_code_reviewer(),
        ),
    ] {
        let w = rig.prepare_as(&profile, role);
        let (_, text) = rig.file_of(dir, "settings.json");
        let v: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(v["autoMemoryEnabled"], false, "{dir}: {text}");
        assert_eq!(v["permissions"]["deny"], json!(["Agent"]), "{dir}: {text}");
        assert!(
            w.env
                .contains(&("CLAUDE_CODE_DISABLE_AUTO_MEMORY".to_owned(), "1".to_owned())),
            "{dir}: {:?}",
            w.env
        );
    }
}

#[test]
fn inherited_claude_code_variables_are_dropped_unless_the_profile_sets_them() {
    let rig = Rig::new();
    let cx_names = [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_MINE",
        "HOME",
    ]
    .map(str::to_owned);
    let mut profile = AgentProfile::claude_code();
    profile.env.insert("CLAUDE_CODE_MINE".into(), "1".into());
    let cx = Context {
        dir: &rig.dir.path().join("runs").join("t1"),
        key: &rig.key,
        role: AgentRole::Worker,
        run: rig.run,
        token: TOKEN,
        url: None,
        bin: &rig.bin,
        brief: &rig.brief,
    };
    let w = prepare_with(&profile, &cx, cx_names).expect("prepare");
    assert_eq!(w.env_remove, ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"]);
}

/// The app lives under `/Applications/Toknila.app` or in a folder with spaces: the hook and the
/// status line commands are run by a shell, so they must still reach the binary with their
/// arguments intact, and the MCP config names it as one `command` string.
#[cfg(unix)]
#[test]
fn a_binary_path_with_spaces_works_in_hooks_status_line_and_mcp_config() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new();
    let bin_dir = rig.dir.path().join("My Apps/Toknila.app/Contents/MacOS");
    std::fs::create_dir_all(&bin_dir).expect("bin dir");
    rig.bin = bin_dir.join("toknila");
    std::fs::write(&rig.bin, "#!/bin/sh\nprintf '%s|' \"$@\"\n").expect("script");
    std::fs::set_permissions(&rig.bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    let run = |command: &str| {
        let out = std::process::Command::new("sh")
            .args(["-c", command])
            .output()
            .expect("sh");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let hook = v["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .expect("stop hook");
    assert_eq!(run(hook), "hook|stop|");
    let status = v["statusLine"]["command"].as_str().expect("status line");
    assert_eq!(run(status), "statusline|");
    let (_, mcp) = rig.file("mcp.json");
    let m: Value = serde_json::from_str(&mcp).expect("mcp json");
    assert_eq!(
        m["mcpServers"]["toknila"]["command"].as_str(),
        Some(rig.bin.to_str().expect("utf8"))
    );
}
