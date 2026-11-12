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
        epics: vec![toknila_core::instructions::Epic {
            chain: "checkout".to_owned(),
            version: Some("0.3.0".to_owned()),
            goal: "Pay with saved cards.".to_owned(),
        }],
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
    assert!(
        text.contains("### checkout (v0.3.0)\n\nPay with saved cards."),
        "the planner reads each epic's goal: {text}"
    );
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
    for want in [
        "Read(//w/ships/demo-1/**)",
        "Bash(git status *)",
        "mcp__toknila",
    ] {
        assert!(allow.contains(&want), "{want}: {allow:?}");
    }
    for bare in ["Read", "Grep", "Glob"] {
        assert!(!allow.contains(&bare), "{bare} is scoped: {allow:?}");
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

/// A rig whose project has two design files and a token file in its data home.
fn rig_with_designs() -> (Rig, Vec<std::path::PathBuf>) {
    let mut rig = planner_rig();
    let folder = rig.dir.path().join("designs").join("p1");
    std::fs::create_dir_all(&folder).expect("designs folder");
    std::fs::create_dir_all(rig.dir.path().join("secrets")).expect("secrets");
    std::fs::write(rig.dir.path().join("secrets").join("figma.token"), "t").expect("token");
    let files: Vec<_> = ["a.png", "b.pdf"].iter().map(|n| folder.join(n)).collect();
    for f in files.iter().chain([&folder.join("index.json")]) {
        std::fs::write(f, b"x").expect("write");
    }
    rig.brief.designs = files.clone();
    (rig, files)
}

fn settings_of(rig: &Rig) -> Value {
    let (_, text) = rig.file_of("planner", "settings.json");
    serde_json::from_str(&text).expect("json")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("array")
        .iter()
        .filter_map(|s| s.as_str().map(str::to_owned))
        .collect()
}

#[test]
fn the_planner_may_read_each_design_file_and_nothing_else_outside_the_repository() {
    let (rig, files) = rig_with_designs();
    rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let s = settings_of(&rig);
    let allow = strings(&s["permissions"]["allow"]);
    // One explicit rule per file, spelled the way Claude Code's rules spell an absolute path.
    let read_rules: Vec<&String> = allow
        .iter()
        .filter(|r| r.starts_with("Read(") && r.contains("designs"))
        .collect();
    let want: Vec<String> = files
        .iter()
        .map(|f| toknila_core::private::read_allow_rule(&f.display().to_string()))
        .collect();
    assert_eq!(read_rules, want.iter().collect::<Vec<_>>(), "{allow:?}");
    assert!(want[0].starts_with("Read(//"), "{}", want[0]);
    assert!(
        !allow
            .iter()
            .any(|r| r.contains("designs") && r.contains("**"))
    );
    // Everything else of the data home stays denied, and so does the rest of the designs folder;
    // the design files are not denied.
    let deny = strings(&s["permissions"]["deny"]);
    // Spelled as the rules are (a Windows drive reads `//c/...`), not as the raw path.
    let denied = |p: &std::path::Path| {
        deny.contains(&toknila_core::private::read_allow_rule(
            &p.display().to_string(),
        ))
    };
    let dir = rig.dir.path();
    assert!(denied(&dir.join("secrets")), "{deny:?}");
    assert!(denied(&dir.join("designs/p1/index.json")), "{deny:?}");
    for f in &files {
        assert!(!denied(f), "{f:?} must stay readable");
    }
}

#[test]
fn a_planner_without_designs_gets_no_design_rules_and_a_plain_kickoff() {
    let rig = planner_rig();
    let w = rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let allow = strings(&settings_of(&rig)["permissions"]["allow"]);
    assert!(!allow.iter().any(|r| r.contains("designs")), "{allow:?}");
    assert!(!w.command.last().expect("kickoff").contains("design"));
}

#[test]
fn the_kickoff_of_a_planner_with_designs_says_to_look_at_them_first() {
    let (rig, _) = rig_with_designs();
    let w = rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let kickoff = w.command.last().expect("kickoff");
    assert!(kickoff.contains("Read every design file"), "{kickoff}");
    assert!(kickoff.contains("name the designs each card"), "{kickoff}");
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
        repo: &rig.repo,
        brief: &rig.brief,
    };
    let e = super::prepare(&AgentProfile::claude_code_planner(), &cx).expect_err("no facts");
    assert!(e.to_string().contains("project's facts"), "{e}");
}

#[test]
fn a_design_added_after_the_start_is_not_read_through_a_rule_and_the_data_home_stays_denied() {
    // The planner started with one design; the owner uploads another and a second project has one.
    let (rig, files) = rig_with_designs();
    let home = rig.dir.path();
    for d in ["db", "runs/t1", "runs/t2", "designs/p2"] {
        std::fs::create_dir_all(home.join(d)).expect("mkdir");
    }
    std::fs::write(home.join("designs/p2/theirs.png"), b"x").expect("write");
    rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let late = home.join("designs/p1/late.png");
    std::fs::write(&late, b"x").expect("a design added after the start");

    let s = settings_of(&rig);
    let allow = strings(&s["permissions"]["allow"]);
    let deny = strings(&s["permissions"]["deny"]);
    let rule =
        |p: &std::path::Path| toknila_core::private::read_allow_rule(&p.display().to_string());
    // The rules did not widen: the late file has no allow rule, and no folder of designs is allowed.
    assert!(!allow.contains(&rule(&late)), "{allow:?}");
    assert!(allow.contains(&rule(&files[0])), "{allow:?}");
    assert!(
        !allow
            .iter()
            .any(|r| r.contains("designs") && r.contains("**"))
    );
    // Denied: the database, a sibling run, the library's index, another project's designs.
    for denied in [
        home.join("db"),
        home.join("runs/t2"),
        home.join("designs/p1/index.json"),
        home.join("designs/p2"),
    ] {
        assert!(deny.contains(&rule(&denied)), "{denied:?} in {deny:?}");
    }
    // Control: the run's own folder and the kept design are not denied.
    for open in [home.join("runs/t1"), files[0].clone()] {
        assert!(!deny.contains(&rule(&open)), "{open:?} must stay open");
    }
}
