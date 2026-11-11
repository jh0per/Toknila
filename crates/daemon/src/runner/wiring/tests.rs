use std::path::PathBuf;

use serde_json::Value;
use toknila_app::runner::RunBrief;
use toknila_core::hook::HOOKS;
use toknila_core::{AgentProfile, AgentRole, RunId};

use super::*;

mod planner;
mod reviewer_rules;

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
                earlier: Vec::new(),
                model: None,
                project: None,
                ..RunBrief::default()
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
fn a_card_agent_reads_the_goal_of_its_chain_in_its_instructions() {
    let mut rig = Rig::new();
    rig.brief.epic = Some(("checkout".to_owned(), "Pay with saved cards.".to_owned()));
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("instructions.md");
    assert!(
        text.contains("## Epic: checkout\n\nThis card is a task of the chain checkout."),
        "{text}"
    );
    assert!(text.contains("Pay with saved cards."));
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
    for must in [
        "run the check command",
        "quote their result lines",
        "NOT_RUN",
    ] {
        assert!(r.command[3].contains(must), "{must} in {}", r.command[3]);
    }
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

/// The value after `--model` in the flags and the command of a wiring, if any.
fn model_of(w: &Wiring) -> Option<String> {
    let all: Vec<&String> = w.command.iter().chain(&w.args).collect();
    all.iter()
        .position(|a| a.as_str() == "--model")
        .map(|i| all[i + 1].clone())
}

#[test]
fn the_project_model_is_passed_as_model_to_every_claude_code_role_and_nowhere_else() {
    let mut rig = Rig::new();
    rig.brief.model = Some("opus".to_owned());
    let worker = rig.prepare_as(&AgentProfile::claude_code(), AgentRole::Worker);
    assert_eq!(model_of(&worker).as_deref(), Some("opus"), "{worker:?}");
    assert!(
        worker.args.windows(2).any(|p| p == ["--model", "opus"]),
        "one flag, one value: {:?}",
        worker.args
    );
    let reviewer = rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    assert_eq!(model_of(&reviewer).as_deref(), Some("opus"));
    // A profile of another program, and the simulated one, get no flag they do not know.
    let other = AgentProfile::new("other", AgentRole::Worker, vec!["my-agent".to_owned()]);
    assert_eq!(model_of(&rig.prepare(&other)), None);
    assert_eq!(model_of(&rig.prepare(&AgentProfile::sim())), None);
    // Control: without a project model nothing is added.
    rig.brief.model = None;
    assert_eq!(model_of(&rig.prepare(&AgentProfile::claude_code())), None);
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

/// The deny rules of every role, written out: a change here is a decision, not a refactor.
const DENIED: [&str; 6] = [
    "Agent",
    "Workflow",
    "EnterWorktree",
    "CronCreate",
    "CronDelete",
    "RemoteTrigger",
];

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
        json!({ "autoMemoryEnabled": false, "permissions": { "deny": denied_with_private() }}),
        "{text}"
    );
}

/// [`DENIED`] followed by the rules for the person's private folders (no data-home entries: the
/// rig's data home holds only the run folder).
fn denied_with_private() -> Vec<String> {
    let home = dirs::home_dir().expect("home").display().to_string();
    DENIED
        .iter()
        .map(|s| (*s).to_owned())
        .chain(toknila_core::private::deny_rules(&home, &[]))
        .collect()
}

#[test]
fn every_role_denies_read_grep_glob_and_bash_on_the_private_folders() {
    let rig = Rig::new();
    let home = dirs::home_dir().expect("home").display().to_string();
    let other = rig.dir.path().join("tokens");
    std::fs::create_dir_all(&other).expect("mkdir");
    std::fs::create_dir_all(rig.dir.path().join("runs").join("t2")).expect("mkdir");
    for (role, dir, profile) in [
        (AgentRole::Worker, "worker", AgentProfile::claude_code()),
        (
            AgentRole::Reviewer,
            "reviewer",
            AgentProfile::claude_code_reviewer(),
        ),
    ] {
        // The planner needs a project's facts; its own test (`planner.rs`) checks its deny list.
        rig.prepare_as(&profile, role);
        let (_, text) = rig.file_of(dir, "settings.json");
        let v: Value = serde_json::from_str(&text).expect("json");
        let deny: Vec<&str> = v["permissions"]["deny"]
            .as_array()
            .expect("deny")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let mut private = toknila_core::private::private_paths(&home);
        private.push(other.display().to_string());
        private.push(rig.dir.path().join("runs").join("t2").display().to_string());
        // The rules are spelled in Claude Code's rooted form (`//c/Users/...` on Windows), so
        // they are compared as rules, not as the raw paths.
        for path in &private {
            for rule in toknila_core::private::deny_rules_for(path, &home) {
                assert!(
                    deny.contains(&rule.as_str()),
                    "{dir}: {rule} missing in {deny:?}"
                );
            }
        }
        // The run's own folder and the workspace stay open.
        let own = rig.dir.path().join("runs").join("t1").display().to_string();
        assert!(!deny.iter().any(|r| r.contains(&own)), "{dir}: {deny:?}");
        assert!(deny.contains(&"Agent"), "{dir}");
    }
}

#[test]
fn every_role_runs_without_auto_memory_and_without_the_denied_tools() {
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
        assert_eq!(
            v["permissions"]["deny"],
            json!(denied_with_private()),
            "{dir}: {text}"
        );
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

/// The permission mode in the command and the allow rules in the settings file of a worker run
/// under `policy`.
fn mode_and_allow(
    policy: toknila_core::CommandPolicy,
    remembered: &[&str],
) -> (String, Vec<String>) {
    let mut rig = Rig::new();
    rig.brief.command_policy = policy;
    rig.brief.allowed_commands = remembered.iter().map(|r| (*r).to_owned()).collect();
    rig.brief.check_command = Some("make check".to_owned());
    let w = rig.prepare(&AgentProfile::claude_code());
    let at = w.command.iter().position(|a| a == "--permission-mode");
    let mode = w.command[at.expect("flag") + 1].clone();
    let (_, text) = rig.file("settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    let rules = |key: &str| -> Vec<String> {
        v["permissions"][key]
            .as_array()
            .expect("rules")
            .iter()
            .map(|r| r.as_str().expect("rule").to_owned())
            .collect()
    };
    assert_eq!(
        rules("deny")[..DENIED_TOOLS.len()],
        DENIED_TOOLS,
        "denied in every mode"
    );
    (mode, rules("allow"))
}

#[test]
fn the_commands_setting_of_the_project_picks_the_mode_and_the_allow_rules() {
    use toknila_core::CommandPolicy::*;
    let (mode, allow) = mode_and_allow(Ask, &["Bash(cargo test *)"]);
    assert_eq!(mode, "acceptEdits");
    assert!(
        allow.contains(&"Bash(cargo test *)".to_owned()),
        "{allow:?}"
    );
    assert!(allow.contains(&"Bash(make check *)".to_owned()));
    assert!(!allow.contains(&"Bash".to_owned()));

    let (mode, allow) = mode_and_allow(Auto, &[]);
    assert_eq!(mode, "auto");
    assert!(!allow.contains(&"Bash".to_owned()));

    let (mode, allow) = mode_and_allow(Trust, &[]);
    assert_eq!(mode, "acceptEdits");
    for rule in ["Bash", "WebFetch", "WebSearch"] {
        assert!(allow.contains(&rule.to_owned()), "{rule}: {allow:?}");
    }
}

/// The allow rules in the settings file of a reviewer run for a project that lists `domains`.
fn reviewer_allow(domains: &[&str]) -> Vec<String> {
    let mut rig = Rig::new();
    rig.brief.web_domains = domains.iter().map(|d| (*d).to_owned()).collect();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, text) = rig.file_of("reviewer", "settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    v["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .map(|r| r.as_str().expect("rule").to_owned())
        .collect()
}

#[test]
fn the_reviewer_may_fetch_the_listed_domains_and_no_others() {
    let allow = reviewer_allow(&["docs.rs", "example.com"]);
    for rule in ["WebFetch(domain:docs.rs)", "WebFetch(domain:example.com)"] {
        assert!(allow.contains(&rule.to_owned()), "{rule}: {allow:?}");
    }
    assert!(
        !allow
            .iter()
            .any(|r| r == "WebFetch" || r == "WebSearch" || r.contains("other.org")),
        "{allow:?}"
    );
    let none = reviewer_allow(&[]);
    assert!(!none.iter().any(|r| r.starts_with("WebFetch")), "{none:?}");
}

/// The allow rules in the settings file of a reviewer for a project with `check` and a repository
/// that holds `repo_tools`.
fn reviewer_allow_for(check: Option<&str>, repo_tools: &[&str]) -> Vec<String> {
    let mut rig = Rig::new();
    rig.brief.check_command = check.map(str::to_owned);
    rig.brief.repo_tools = repo_tools.iter().map(|t| (*t).to_owned()).collect();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, text) = rig.file_of("reviewer", "settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    v["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .map(|r| r.as_str().expect("rule").to_owned())
        .collect()
}

#[test]
fn a_cargo_reviewer_may_run_cargo_test_xtask_clippy_and_the_check_but_not_write() {
    let allow = reviewer_allow_for(Some("cargo xtask verify"), &["cargo"]);
    for want in [
        "Bash(cargo test *)",
        "Bash(cargo xtask *)",
        "Bash(cargo clippy *)",
        "Bash(cargo xtask verify *)",
        "Bash(git diff *)",
    ] {
        assert!(allow.contains(&want.to_owned()), "{want}: {allow:?}");
    }
    for never in [
        "git commit",
        "git push",
        "git checkout",
        "git add",
        "install",
        "Bash\"",
    ] {
        assert!(
            !allow.iter().any(|r| r.contains(never)),
            "{never}: {allow:?}"
        );
    }
    assert!(!allow.iter().any(|r| r == "Bash"), "{allow:?}");
}

#[test]
fn a_reviewer_without_a_check_command_still_runs_the_tests_the_repository_has() {
    let allow = reviewer_allow_for(None, &["cargo"]);
    assert!(
        allow.contains(&"Bash(cargo test *)".to_owned()),
        "{allow:?}"
    );
    let node = reviewer_allow_for(None, &["npm"]);
    for want in ["Bash(npm test *)", "Bash(npm run *)"] {
        assert!(node.contains(&want.to_owned()), "{want}: {node:?}");
    }
    let bare = reviewer_allow_for(None, &[]);
    assert!(
        !bare
            .iter()
            .any(|r| r.contains("cargo") || r.contains("npm")),
        "{bare:?}"
    );
}

#[test]
fn the_repository_tools_come_from_the_files_of_the_ship() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(repo_tools(dir.path()).is_empty());
    std::fs::write(dir.path().join("Cargo.toml"), "").expect("write");
    std::fs::write(dir.path().join("package.json"), "{}").expect("write");
    std::fs::write(dir.path().join("pnpm-lock.yaml"), "").expect("write");
    assert_eq!(repo_tools(dir.path()), ["cargo", "pnpm"]);
}

#[test]
fn a_worker_gets_no_domain_rules_from_the_reviewers_list() {
    let mut rig = Rig::new();
    rig.brief.web_domains = vec!["docs.rs".to_owned()];
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("settings.json");
    assert!(!text.contains("WebFetch"), "{text}");
}

#[test]
fn a_reviewer_keeps_dont_ask_whatever_the_projects_setting() {
    let mut rig = Rig::new();
    rig.brief.command_policy = toknila_core::CommandPolicy::Trust;
    rig.brief.allowed_commands = vec!["Bash(rm *)".to_owned()];
    let w = rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    assert!(
        w.command
            .windows(2)
            .any(|p| p == ["--permission-mode", "dontAsk"])
    );
    let (_, text) = rig.file_of("reviewer", "settings.json");
    assert!(
        !text.contains("\"Bash\"") && !text.contains("Bash(rm *)"),
        "{text}"
    );
}

/// The shell Claude Code runs hook commands in: Git Bash on Windows (not the WSL `bash` that may
/// come first on `PATH`), the system `sh` elsewhere.
fn hook_shell() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("sh");
    }
    std::env::var_os("GIT_BASH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"))
}

#[test]
fn every_hook_command_runs_under_the_hook_shell_with_a_path_that_has_a_space() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin_dir = tmp.path().join("with space");
    std::fs::create_dir_all(&bin_dir).expect("dir");
    let out = tmp.path().join("out.txt");
    let bin = bin_dir.join("toknila");
    // A stand-in `toknila` that records how it was called.
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\n",
        out.display().to_string().replace('\\', "/")
    );
    std::fs::write(&bin, script).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let settings = hook_settings(&bin);
    let mut commands: Vec<String> = settings["hooks"]
        .as_object()
        .expect("hooks")
        .values()
        .map(|e| {
            e[0]["hooks"][0]["command"]
                .as_str()
                .expect("command")
                .to_owned()
        })
        .collect();
    commands.push(
        settings["statusLine"]["command"]
            .as_str()
            .expect("status")
            .to_owned(),
    );
    for command in &commands {
        let status = std::process::Command::new(hook_shell())
            .arg("-c")
            .arg(command)
            .status()
            .expect("the hook shell starts");
        assert!(status.success(), "{command}: {status}");
    }
    let got = std::fs::read_to_string(&out).expect("the stand-in ran");
    let lines: Vec<&str> = got.lines().map(str::trim).collect();
    assert_eq!(lines.len(), commands.len(), "{got:?}");
    assert!(lines.contains(&"statusline"), "{got:?}");
    assert!(
        lines
            .iter()
            .all(|l| l.starts_with("hook ") || *l == "statusline"),
        "{got:?}"
    );
}
