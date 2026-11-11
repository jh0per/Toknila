//! A project's planner through the real `tok` binary and daemon, with the workspace's
//! `fake-agent` as the planner profile's command (no real agent is launched). The planner runs in
//! the project's repository, plans with its run token (chains, cards, threads), and is refused
//! everything that starts, accepts or stops: those routes are not on its list. `tok mcp` in the
//! planner role does the same through MCP tools.

mod common;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Tok, call, mcp_session, repo};
use serde_json::{Value, json};

/// A ceiling, not a delay: a wait returns the moment its event happens.
const WAIT: Duration = Duration::from_secs(120);

fn planner_logs(tok: &Tok) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["planner", "logs", "DEMO"])
        .env("TOKNILA_HOME", &tok.home)
        .output()
        .expect("run tok planner logs");
    plain(&String::from_utf8_lossy(&out.stdout))
}

/// A terminal's text without its escape sequences: ConPTY draws a line with cursor moves and
/// colour codes around the text, a Unix pty passes the text through.
fn plain(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters, then a final byte from @ to ~.
            Some('[') => {
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            }
            // OSC: up to a BEL or ESC \.
            Some(']') => {
                while let Some(n) = chars.next() {
                    if n == '\u{7}' || (n == '\u{1b}' && chars.next() == Some('\\')) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out.replace('\r', "")
}

fn wait_for(tok: &Tok, what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !ok() {
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}: {}",
            planner_logs(tok)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A project DEMO whose repository is real, and a planner profile with `steps`.
fn setup(steps: &[&str]) -> (tempfile::TempDir, Tok, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    tok.profile(
        "fake-planner",
        steps,
        "role = 'planner'\nstartup_secs = 0\n",
    );
    tok.run(&["project", "new", "DEMO", "--repo", &repo.to_string_lossy()]);
    (dir, tok, repo)
}

#[test]
fn a_planner_plans_with_its_token_and_is_refused_everything_that_starts_or_accepts() {
    let json = |s: &str| s.to_owned();
    let chain = json(r#"{"name":"plan","goal":"the first plan"}"#);
    let one = json(r#"{"title":"One","prompt":"do one","acceptance":["it works"]}"#);
    let two = json(r#"{"title":"Two","prompt":"do two","after":["DEMO-1"]}"#);
    let thread = json(r#"{"kind":"design","title":"Order of work"}"#);
    let (_dir, tok, repo) = setup(&[
        "pwd",
        "env",
        "TOKNILA_ROLE",
        "post",
        "/planner/chains",
        &chain,
        "post",
        "/planner/chains/plan/cards",
        &one,
        "post",
        "/planner/chains/plan/cards",
        &two,
        "post",
        "/tickets/DEMO-1/actions/start",
        "{}",
        "post",
        "/tickets/DEMO-1/actions/accept",
        "{}",
        "post",
        "/tickets/DEMO-1/events",
        r#"{"event":"started"}"#,
        "post",
        "/planner/cards/DEMO-1/threads",
        &thread,
        "post",
        "/projects/DEMO/planner/stop",
        "{}",
        "hold",
    ]);

    let started = tok.run(&["planner", "start", "DEMO", "--profile", "fake-planner"]);
    assert_eq!(started["running"], true);
    wait_for(&tok, "the planner's eight calls", || {
        planner_logs(&tok).matches("post=").count() == 8
    });
    let out = planner_logs(&tok);

    // It runs in the project's repository, in its own role.
    let pwd = out
        .lines()
        .find(|l| l.trim_end().ends_with("repo"))
        .expect("pwd line");
    assert_eq!(
        std::fs::canonicalize(pwd.trim()).expect("repo"),
        std::fs::canonicalize(&repo).expect("repo")
    );
    assert!(out.contains("TOKNILA_ROLE=planner"), "{out}");

    // What its token did, and what it was refused, in order.
    let statuses: Vec<&str> = out
        .lines()
        .filter_map(|l| l.trim().strip_prefix("post="))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(
        statuses,
        ["201", "201", "201", "403", "403", "403", "201", "403"],
        "chain, two cards, start, accept, a card event, a thread, stopping itself\n{out}"
    );

    // The plan changed, and nothing started.
    let chain = tok.run(&["chain", "show", "plan"]);
    assert_eq!(chain["chain"]["autostart"], false);
    assert_eq!(
        tok.statuses(),
        ["DEMO-1 ready", "DEMO-2 backlog"],
        "the refused start did not start DEMO-1"
    );
    let threads = tok.run(&["thread", "ls", "DEMO-1"]);
    assert_eq!(threads["threads"][0]["thread"]["opened_by"], "agent");
    assert_eq!(threads["phase"], "design");
    assert_eq!(tok.run(&["planner", "start", "DEMO"])["running"], true);

    // The owner stops it; a second stop is fine.
    assert_eq!(
        tok.text(&["planner", "stop", "DEMO"]),
        "planner of DEMO stopped\n"
    );
    wait_for(&tok, "the planner to stop", || {
        !tok.client().planner("DEMO").expect("state").running
    });
    assert_eq!(
        tok.text(&["planner", "stop", "DEMO"]),
        "planner of DEMO was not running\n"
    );
    assert_eq!(tok.run(&["planner", "stop", "DEMO"])["running"], false);
    assert!(planner_logs(&tok).contains("post="), "history stays");
}

#[test]
fn only_a_planner_profile_in_a_project_with_a_repository_starts_a_planner() {
    let (_dir, tok, _) = setup(&["hold"]);
    let msg = tok.fails(&["planner", "start", "DEMO", "--profile", "sim"]);
    assert!(msg.contains("not a planner profile"), "{msg}");
    let msg = tok.fails(&["planner", "start", "DEMO", "--profile", "nope"]);
    assert!(msg.contains("does not exist"), "{msg}");
    tok.run(&["project", "new", "BARE"]);
    let msg = tok.fails(&["planner", "start", "BARE", "--profile", "fake-planner"]);
    assert!(msg.contains("no repository"), "{msg}");
    assert!(!tok.client().planner("DEMO").expect("state").running);
}

fn text_of(answer: &Value) -> String {
    answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_planner_tools_edit_the_plan_through_the_daemon_and_stop_at_the_owners_calls() {
    let (_dir, tok, _) = setup(&["env", "TOKNILA_RUN_TOKEN", "hold"]);
    // A chain the owner made the ordinary way: it starts its cards by itself.
    tok.run(&["chain", "new", "auto"]);
    tok.run(&["planner", "start", "DEMO", "--profile", "fake-planner"]);
    wait_for(&tok, "the token line", || {
        planner_logs(&tok).contains("TOKNILA_RUN_TOKEN=")
    });
    let token = planner_logs(&tok)
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("TOKNILA_RUN_TOKEN=")
                .map(str::to_owned)
        })
        .expect("token");

    let mut lines = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        call(
            3,
            "new_chain",
            json!({ "name": "mcp", "goal": "planned by tools" }),
        ),
        call(
            4,
            "add_card",
            json!({ "chain": "mcp", "title": "First", "prompt": "do it",
                    "acceptance": ["it works"], "subtasks": ["one"] }),
        ),
        call(5, "add_card", json!({ "chain": "mcp", "title": "Second" })),
        call(
            6,
            "add_card",
            json!({ "chain": "mcp", "title": "Third", "after": ["DEMO-1"] }),
        ),
        call(
            7,
            "move_card",
            json!({ "chain": "mcp", "card": "DEMO-3", "anchor": "DEMO-1", "place": "before" }),
        ),
        call(
            8,
            "remove_dependency",
            json!({ "chain": "mcp", "from": "DEMO-3", "to": "DEMO-1" }),
        ),
        call(
            9,
            "add_dependency",
            json!({ "chain": "mcp", "from": "DEMO-3", "to": "DEMO-2" }),
        ),
        call(
            10,
            "edit_card",
            json!({ "card": "DEMO-1", "title": "First, renamed", "add_subtasks": ["two"],
                    "edit_subtasks": [{ "position": 1, "text": "uno" }] }),
        ),
        call(
            11,
            "open_thread",
            json!({ "card": "DEMO-1", "kind": "flow", "title": "Flow" }),
        ),
        call(
            12,
            "thread_message",
            json!({ "card": "DEMO-1", "thread": "1", "text": "Which first?" }),
        ),
        call(
            13,
            "thread_version",
            json!({ "card": "DEMO-1", "thread": "1", "format": "mermaid", "body": "graph TD\n A-->B" }),
        ),
        // A chain that starts its cards by itself refuses what could start work.
        call(
            14,
            "add_card",
            json!({ "chain": "auto", "title": "Sneaky" }),
        ),
        call(15, "show_plan", json!({})),
        call(19, "show_card", json!({ "card": "DEMO-2" })),
        call(20, "show_card", json!({ "card": "DEMO-99" })),
        // The owner's calls are not tools.
        call(16, "start", json!({ "card": "DEMO-1" })),
        call(17, "accept", json!({ "card": "DEMO-1" })),
        call(18, "finished", json!({ "summary": "x" })),
    ];
    let answers = mcp_session(
        &tok.info().base_url(),
        &token,
        &[("TOKNILA_ROLE", "planner")],
        &lines,
    );
    lines.clear();
    let by_id = |id: u64| {
        answers
            .iter()
            .find(|a| a["id"] == id)
            .unwrap_or_else(|| panic!("no answer {id}: {answers:?}"))
    };

    assert_eq!(
        by_id(1)["result"]["instructions"],
        toknila_core::instructions::planner_protocol()
    );
    let names: Vec<&str> = by_id(2)["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "show_plan",
            "new_chain",
            "add_card",
            "edit_card",
            "show_card",
            "add_dependency",
            "remove_dependency",
            "move_card",
            "open_thread",
            "thread_message",
            "thread_version"
        ]
    );
    assert!(text_of(by_id(3)).contains("chain mcp created"));
    assert!(
        text_of(by_id(4)).contains("DEMO-1 added in chain mcp"),
        "{}",
        text_of(by_id(4))
    );
    // Second goes after the first by default (the end of the chain); the third was told to wait
    // for the first, and moving it before the first reorders them.
    assert!(
        text_of(by_id(7)).contains("DEMO-3 < DEMO-1 < DEMO-2"),
        "{}",
        text_of(by_id(7))
    );
    assert!(
        text_of(by_id(9)).starts_with("chain mcp:"),
        "{}",
        text_of(by_id(9))
    );
    for id in [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13] {
        assert_eq!(
            by_id(id)["result"]["isError"],
            false,
            "call {id}: {}",
            text_of(by_id(id))
        );
    }
    assert_eq!(
        by_id(19)["result"]["isError"],
        false,
        "{}",
        text_of(by_id(19))
    );
    assert!(
        text_of(by_id(19)).starts_with("DEMO-2 shown in the window"),
        "{}",
        text_of(by_id(19))
    );
    assert_eq!(by_id(20)["result"]["isError"], true);
    assert_eq!(by_id(14)["result"]["isError"], true);
    assert!(
        text_of(by_id(14)).contains("only the owner starts cards"),
        "{}",
        text_of(by_id(14))
    );
    for id in [16, 17, 18] {
        assert!(
            by_id(id)["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("unknown tool")
        );
    }

    let plan: Value = serde_json::from_str(&text_of(by_id(15))).expect("the plan is JSON");
    assert_eq!(plan["project"]["key"], "DEMO");
    let mcp_chain = plan["chains"]
        .as_array()
        .expect("chains")
        .iter()
        .find(|c| c["chain"]["name"] == "mcp")
        .expect("the chain the tools made");
    assert_eq!(mcp_chain["chain"]["autostart"], false);
    let keys: Vec<&str> = mcp_chain["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .filter_map(|c| c["ticket"]["key"].as_str())
        .collect();
    assert_eq!(keys.len(), 3);
    let first = mcp_chain["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|c| c["ticket"]["key"] == "DEMO-1")
        .expect("DEMO-1");
    assert_eq!(first["ticket"]["title"], "First, renamed");
    assert_eq!(first["subtasks"].as_array().map(Vec::len), Some(2));
    assert_eq!(first["subtasks"][0]["text"], "uno");
    assert_eq!(first["threads"][0]["versions"], 1);
    assert_eq!(first["threads"][0]["messages"][0]["author"], "agent");

    // Nothing the tools did started a card, and the sneaky card was never made.
    assert!(
        tok.statuses()
            .iter()
            .all(|s| s.ends_with("ready") || s.ends_with("backlog")),
        "{:?}",
        tok.statuses()
    );
    assert_eq!(tok.statuses().len(), 3);
    tok.run(&["planner", "stop", "DEMO"]);
}
