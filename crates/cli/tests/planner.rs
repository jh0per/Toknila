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
    let dir = common::tempdir();
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

/// Ten start/stop cycles of a fake planner: `tok planner stop` is one short call each time, and
/// none of them may fail on the transport, however loaded the machine is.
#[test]
fn ten_start_stop_cycles_of_a_fake_planner_never_fail_a_stop() {
    let (_dir, tok, _repo) = setup(&["hold"]);
    for cycle in 0..10 {
        assert_eq!(
            tok.run(&["planner", "start", "DEMO"])["running"],
            true,
            "start {cycle}"
        );
        assert_eq!(
            tok.text(&["planner", "stop", "DEMO"]),
            "planner of DEMO stopped\n",
            "stop {cycle}"
        );
        wait_for(&tok, "the planner to stop", || {
            !tok.client().planner("DEMO").expect("state").running
        });
    }
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

    // A design of the project, kept the way the owner's upload keeps it.
    let png: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R',
    ];
    let info = tok.info();
    let url = format!("{}/v1/projects/DEMO/designs?name=home.png", info.base_url());
    ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", info.token))
        .send_bytes(png)
        .expect("upload a design");

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
        call(21, "show_design", json!({ "design": "home.png" })),
        call(22, "show_design", json!({ "design": "nope.png" })),
        call(23, "suggest", json!({ "text": "Nothing waits for you" })),
        call(
            24,
            "suggest",
            json!({ "text": "x", "card": "DEMO-1", "action": "accept" }),
        ),
        call(
            25,
            "design_question",
            json!({ "design": "home.png", "region": { "x": 10, "y": 20, "w": 30, "h": 40 },
                    "text": "Is the logo fixed?", "card": "DEMO-1" }),
        ),
        call(26, "show_plan", json!({})),
        call(27, "show_plan", json!({ "chain": "mcp" })),
        call(28, "show_plan", json!({ "card": "DEMO-1" })),
        call(29, "show_plan", json!({ "card": "DEMO-99" })),
        call(30, "show_plan", json!({ "chain": "mcp", "card": "DEMO-1" })),
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
            "show_design",
            "design_question",
            "new_chain",
            "edit_chain",
            "add_card",
            "edit_card",
            "show_card",
            "suggest",
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
    assert_eq!(
        by_id(23)["result"]["isError"],
        false,
        "{}",
        text_of(by_id(23))
    );
    assert_eq!(text_of(by_id(23)), "next step kept");
    assert_eq!(
        by_id(24)["result"]["isError"],
        true,
        "DEMO-1 is not in review"
    );
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
    // show_plan lists the design with the path of its file, and show_design answers the same.
    let designs = plan["designs"].as_array().expect("designs");
    assert_eq!(designs.len(), 1, "{plan}");
    let path = designs[0]["path"].as_str().expect("path");
    assert!(std::path::Path::new(path).is_file(), "{path}");
    assert!(path.starts_with(&*tok.home.to_string_lossy()), "{path}");
    assert_eq!(designs[0]["name"], "home.png");
    let one: Value = serde_json::from_str(&text_of(by_id(21))).expect("the design is JSON");
    assert_eq!(one, designs[0]);
    assert_eq!(by_id(22)["result"]["isError"], true, "{}", by_id(22));
    assert!(text_of(by_id(22)).contains("nope.png"), "{}", by_id(22));
    // design_question pins on the design; show_plan then lists the pin, open, with its place.
    assert!(
        text_of(by_id(25)).contains("pinned #1 on home.png"),
        "{}",
        text_of(by_id(25))
    );
    let after: Value = serde_json::from_str(&text_of(by_id(26))).expect("the plan is JSON");
    assert_eq!(plan["pins"].as_array().map(Vec::len), Some(0), "{plan}");
    let pins = after["pins"].as_array().expect("pins");
    assert_eq!(pins.len(), 1, "{after}");
    assert_eq!(pins[0]["text"], "Is the logo fixed?");
    assert_eq!(pins[0]["card"], "DEMO-1");
    assert_eq!(pins[0]["region"]["w"], 30);
    assert_eq!(pins[0]["answer"], Value::Null);
    // The overview names the chain the tools made and its three cards in short.
    let short = plan["chains"]
        .as_array()
        .expect("chains")
        .iter()
        .find(|c| c["name"] == "mcp")
        .expect("the chain the tools made");
    assert_eq!(short["autostart"], false);
    assert_eq!(short["cards"].as_array().map(Vec::len), Some(3), "{short}");
    assert!(short["cards"][0].get("prompt").is_none(), "{short}");
    // `chain` opens it in full, `card` opens one card in full.
    let mcp_chain: Value = serde_json::from_str(&text_of(by_id(27))).expect("the chain is JSON");
    assert_eq!(mcp_chain["chain"]["name"], "mcp");
    let keys: Vec<&str> = mcp_chain["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .filter_map(|c| c["ticket"]["key"].as_str())
        .collect();
    assert_eq!(keys.len(), 3);
    let one: Value = serde_json::from_str(&text_of(by_id(28))).expect("the card is JSON");
    assert_eq!(one["ticket"]["key"], "DEMO-1");
    assert_eq!(one["subtasks"].as_array().map(Vec::len), Some(2), "{one}");
    assert_eq!(by_id(29)["result"]["isError"], true, "{}", by_id(29));
    assert_eq!(by_id(30)["result"]["isError"], true, "{}", by_id(30));
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

#[test]
fn show_plan_without_arguments_stays_small_for_a_project_of_a_hundred_big_cards() {
    let (_dir, tok, _) = setup(&["env", "TOKNILA_RUN_TOKEN", "hold"]);
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
        call(
            1,
            "new_chain",
            json!({ "name": "mini", "goal": "two cards" }),
        ),
        call(
            2,
            "add_card",
            json!({ "chain": "mini", "title": "One", "prompt": "p" }),
        ),
        call(
            3,
            "add_card",
            json!({ "chain": "mini", "title": "Two", "prompt": "p", "after": ["DEMO-1"],
                    "subtasks": ["a"], "design_first": true }),
        ),
        call(
            4,
            "new_chain",
            json!({ "name": "big", "goal": "a hundred cards" }),
        ),
    ];
    let prompt = "x".repeat(3_000);
    for n in 1..=100u32 {
        let mut card = json!({ "chain": "big", "title": format!("Card {n}"), "prompt": prompt,
                               "acceptance": ["a".repeat(200)] });
        if n > 1 {
            card["after"] = json!([format!("DEMO-{}", n + 1)]);
        }
        lines.push(call(10 + n, "add_card", card));
    }
    lines.push(call(200, "show_plan", json!({})));
    lines.push(call(201, "show_plan", json!({ "chain": "big" })));
    let answers = mcp_session(
        &tok.info().base_url(),
        &token,
        &[("TOKNILA_ROLE", "planner")],
        &lines,
    );
    let by_id = |id: u64| {
        answers
            .iter()
            .find(|a| a["id"] == id)
            .unwrap_or_else(|| panic!("no answer {id}"))
    };
    let overview = text_of(by_id(200));
    assert!(
        overview.len() < 40_000,
        "the overview of 100 cards with 3,000-character prompts is {} characters",
        overview.len()
    );
    let parsed: Value = serde_json::from_str(&overview).expect("the overview is JSON");
    let big = parsed["chains"]
        .as_array()
        .expect("chains")
        .iter()
        .find(|c| c["name"] == "big")
        .expect("big");
    assert_eq!(big["cards"].as_array().map(Vec::len), Some(100));
    assert_eq!(big["cards"][99]["key"], "DEMO-102");
    assert_eq!(big["cards"][99]["status"], "backlog");
    assert_eq!(big["cards"][99]["after"][0], "DEMO-101");
    // The control: the chain in full is the big answer, so the ceiling measures something.
    assert!(text_of(by_id(201)).len() > 300_000);
    let mini = parsed["chains"]
        .as_array()
        .expect("chains")
        .iter()
        .find(|c| c["name"] == "mini")
        .expect("mini")
        .clone();
    common::snapshot("planner_overview_chain", mini);
    tok.run(&["planner", "stop", "DEMO"]);
}

#[test]
fn show_design_gives_the_picture_of_a_design_added_after_the_planner_started() {
    let (_dir, tok, _) = setup(&["env", "TOKNILA_RUN_TOKEN", "hold"]);
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

    // Uploaded while the planner runs: its run was started without any design.
    let png: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R',
    ];
    let info = tok.info();
    let upload = |path: &str, body: &[u8], content_type: &str| {
        ureq::post(&format!("{}/v1/projects/{path}", info.base_url()))
            .set("Authorization", &format!("Bearer {}", info.token))
            .set("Content-Type", content_type)
            .send_bytes(body)
            .expect("upload");
    };
    upload(
        "DEMO/designs?name=late.png",
        png,
        "application/octet-stream",
    );
    let link = json!({ "url": "https://www.figma.com/design/abc123/App?node-id=1-2",
                       "name": "Checkout" })
    .to_string();
    upload("DEMO/designs/figma", link.as_bytes(), "application/json");
    tok.run(&["project", "new", "OTHER"]);
    upload(
        "OTHER/designs?name=theirs.png",
        png,
        "application/octet-stream",
    );

    let answers = mcp_session(
        &info.base_url(),
        &token,
        &[("TOKNILA_ROLE", "planner")],
        &[
            call(1, "show_design", json!({ "design": "late.png" })),
            call(2, "show_design", json!({ "design": "Checkout" })),
            call(3, "show_design", json!({ "design": "theirs.png" })),
        ],
    );
    let by_id = |id: u64| answers.iter().find(|a| a["id"] == id).expect("answer");
    let content = by_id(1)["result"]["content"].as_array().expect("content");
    assert_eq!(content.len(), 2, "{content:?}");
    let meta: Value = serde_json::from_str(&text_of(by_id(1))).expect("metadata is JSON");
    assert_eq!(meta["name"], "late.png");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["mimeType"], "image/png");
    // The well-known base64 of the 16 bytes of a png header.
    assert_eq!(content[1]["data"], "iVBORw0KGgoAAAANSUhEUg==");

    // A link without a fetched image has no file, and the answer says so in words.
    let figma = by_id(2)["result"]["content"].as_array().expect("content");
    assert_eq!(by_id(2)["result"]["isError"], false);
    assert_eq!(figma.len(), 2, "{figma:?}");
    assert_eq!(figma[1]["type"], "text");
    assert!(
        figma[1]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Checkout has no file"),
        "{figma:?}"
    );

    // Another project's design is refused.
    assert_eq!(by_id(3)["result"]["isError"], true, "{}", by_id(3));
    tok.run(&["planner", "stop", "DEMO"]);
}

#[test]
fn a_card_takes_the_model_the_assistant_names_and_a_key_no_tool_takes_is_refused() {
    let (_dir, tok, _) = setup(&["env", "TOKNILA_RUN_TOKEN", "hold"]);
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
    let info = tok.info();
    let answers = mcp_session(
        &info.base_url(),
        &token,
        &[("TOKNILA_ROLE", "planner")],
        &[
            call(1, "new_chain", json!({ "name": "m" })),
            call(
                2,
                "add_card",
                json!({ "chain": "m", "title": "Deep", "model": "fable" }),
            ),
            call(
                3,
                "add_card",
                json!({ "chain": "m", "title": "Wrong", "model": "gpt" }),
            ),
            call(
                4,
                "add_card",
                json!({ "chain": "m", "title": "Typo", "modle": "opus" }),
            ),
            call(5, "show_plan", json!({ "card": "DEMO-1" })),
            call(6, "show_plan", json!({})),
            call(7, "edit_card", json!({ "card": "DEMO-1", "model": "gpt" })),
            call(8, "show_plan", json!({ "chain": "nope", "extra": 1 })),
            call(9, "suggest", json!({ "text": "x", "colour": "red" })),
        ],
    );
    let by_id = |id: u64| answers.iter().find(|a| a["id"] == id).expect("answer");
    assert_eq!(by_id(2)["result"]["isError"], false, "{}", by_id(2));
    let one: Value = serde_json::from_str(&text_of(by_id(5))).expect("the card is JSON");
    assert_eq!(one["ticket"]["model"], "fable", "{one}");
    let plan: Value = serde_json::from_str(&text_of(by_id(6))).expect("the plan is JSON");
    let cards = &plan["chains"]
        .as_array()
        .and_then(|c| c.iter().find(|c| c["name"] == "m"))
        .expect("the chain")["cards"];
    assert_eq!(
        cards.as_array().map(Vec::len),
        Some(1),
        "gpt added nothing: {plan}"
    );
    assert_eq!(cards[0]["model"], "fable", "{plan}");
    for id in [3, 7] {
        assert_eq!(by_id(id)["result"]["isError"], true, "{}", by_id(id));
        assert!(
            text_of(by_id(id)).contains("sonnet, opus, haiku, fable"),
            "{}",
            text_of(by_id(id))
        );
    }
    for (id, key) in [(4, "modle"), (8, "extra"), (9, "colour")] {
        assert_eq!(by_id(id)["result"]["isError"], true, "{}", by_id(id));
        assert!(text_of(by_id(id)).contains(key), "{}", text_of(by_id(id)));
    }
    tok.run(&["planner", "stop", "DEMO"]);
}
