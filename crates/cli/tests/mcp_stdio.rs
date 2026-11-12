//! `tok mcp` through the real binary over stdio against a real daemon. An agent's events
//! come in as MCP tool calls and move the ticket exactly as the same events from `tok sim` do.
//!
//! The run token here is the daemon's own token: the daemon only issues agent tokens to runs it
//! starts. What an agent token may and may not do is covered through the HTTP router in
//! `crates/daemon` (`http::tests::agent_tokens`); this test covers the protocol and the CLI.

mod common;

use std::process::{Command, Stdio};

use common::{Tok, call, mcp_session};
use serde_json::{Value, json};
use toknila_daemon::info::DaemonInfo;

const DEMO: &str = include_str!("../../../examples/demo.toml");

/// One MCP session as the agent of `ticket`: the `lines` are sent, stdin is closed, and every
/// answer is returned.
fn session(d: &DaemonInfo, token: &str, ticket: &str, lines: &[Value]) -> Vec<Value> {
    mcp_session(&d.base_url(), token, &[("TOKNILA_TICKET", ticket)], lines)
}

fn handshake() -> Vec<Value> {
    vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    ]
}

#[test]
fn mcp_tool_calls_move_a_ticket_exactly_like_tok_sim() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let file = dir.path().join("demo.toml");
    std::fs::write(&file, DEMO).expect("write demo");
    let file = file.to_str().expect("utf-8 path");
    tok.run(&["project", "new", "DEMO", "--name", "Demo", "--sim"]);
    tok.run(&["chain", "import", file, "--project", "DEMO"]);
    tok.run(&["sim", "DEMO-1", "finish", "A is done"]);
    tok.run(&["accept", "DEMO-1"]);
    // DEMO-2 is driven by `tok sim`, DEMO-3 by MCP tool calls: the same steps, the same results.
    for t in ["DEMO-2", "DEMO-3"] {
        tok.run(&["subtask", "add", t, "write the test"]);
        assert_eq!(tok.status(t), "running");
    }
    let d = tok.info();

    // The protocol: handshake and the tool list.
    let answers = session(&d, &d.token, "DEMO-3", &handshake());
    assert_eq!(
        answers.len(),
        2,
        "the notification gets no answer: {answers:?}"
    );
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "toknila");
    let tools: Vec<&str> = answers[1]["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        tools,
        [
            "started",
            "ask_human",
            "subtask_done",
            "note",
            "finished",
            "crashed",
            "propose",
            "open_thread",
            "thread_message",
            "thread_version",
            "withdraw_thread"
        ]
    );

    // started, ask_human: Waiting with the question as the reason.
    tok.run(&["sim", "DEMO-2", "ask", "which way?"]);
    let answers = session(
        &d,
        &d.token,
        "DEMO-3",
        &[
            call(1, "started", json!({})),
            call(2, "ask_human", json!({ "text": "which way?" })),
        ],
    );
    assert_eq!(answers[0]["result"]["isError"], false, "{answers:?}");
    assert_eq!(
        answers[1]["result"]["content"][0]["text"],
        "DEMO-3 is waiting"
    );
    for t in ["DEMO-2", "DEMO-3"] {
        let s = tok.run(&["show", t])["ticket"].clone();
        assert_eq!(s["status"], "waiting", "{t}");
        assert_eq!(s["status_reason"]["text"], "which way?", "{t}");
    }

    // The person answers; the agent ticks, reports and hands in.
    for t in ["DEMO-2", "DEMO-3"] {
        tok.run(&["comment", t, "left"]);
        assert_eq!(tok.status(t), "running", "{t}");
    }
    tok.run(&["sim", "DEMO-2", "subtask", "1", "done"]);
    tok.run(&["sim", "DEMO-2", "note", "half way"]);
    tok.run(&["sim", "DEMO-2", "finish", "did it"]);
    let answers = session(
        &d,
        &d.token,
        "DEMO-3",
        &[
            call(1, "subtask_done", json!({ "position": 1 })),
            call(2, "note", json!({ "body": { "gist": "half way" } })),
            call(
                3,
                "finished",
                json!({ "summary": toknila_core::handin::from_plain("did it") }),
            ),
        ],
    );
    assert!(
        answers.iter().all(|a| a["result"]["isError"] == false),
        "{answers:?}"
    );
    let (sim, mcp) = (tok.run(&["show", "DEMO-2"]), tok.run(&["show", "DEMO-3"]));
    for field in ["status", "status_reason"] {
        assert_eq!(sim["ticket"][field], mcp["ticket"][field], "{field}");
    }
    assert_eq!(mcp["ticket"]["status"], "review");
    assert_eq!(sim["subtasks"][0]["done_by"], "agent");
    assert_eq!(mcp["subtasks"][0]["done_by"], "agent");
    let comments = |v: &Value| -> Vec<Value> {
        v["comments"]
            .as_array()
            .expect("comments")
            .iter()
            .map(|c| json!([c["author"], c["kind"], c["body"]["gist"]]))
            .collect()
    };
    assert_eq!(comments(&sim), comments(&mcp));

    // crashed: Blocked(crashed), after the person returned the ticket to its agent.
    for t in ["DEMO-2", "DEMO-3"] {
        tok.run(&["return", t, "again"]);
    }
    tok.run(&["sim", "DEMO-2", "crash", "exit 101"]);
    let answers = session(
        &d,
        &d.token,
        "DEMO-3",
        &[call(1, "crashed", json!({ "detail": "exit 101" }))],
    );
    assert_eq!(
        answers[0]["result"]["content"][0]["text"],
        "DEMO-3 is blocked"
    );
    let (sim, mcp) = (tok.run(&["show", "DEMO-2"]), tok.run(&["show", "DEMO-3"]));
    assert_eq!(
        sim["ticket"]["status_reason"],
        mcp["ticket"]["status_reason"]
    );
    assert_eq!(mcp["ticket"]["status_reason"]["reason"]["kind"], "crashed");
}

#[test]
fn a_refused_event_and_a_bad_token_come_back_as_tool_errors_and_change_nothing() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--name", "Demo", "--sim"]);
    tok.run(&["chain", "new", "demo", "--goal", "g"]);
    tok.run(&["ticket", "new", "demo", "A"]);
    assert_eq!(tok.status("DEMO-1"), "running");
    let d = tok.info();

    // An open thread refuses the hand-in: the daemon's message reaches the agent.
    let answers = session(
        &d,
        &d.token,
        "DEMO-1",
        &[
            call(
                1,
                "open_thread",
                json!({ "kind": "design", "title": "List" }),
            ),
            call(
                2,
                "thread_version",
                json!({ "thread": "1", "format": "mermaid", "body": "graph TD" }),
            ),
            call(
                3,
                "finished",
                json!({ "summary": toknila_core::handin::from_plain("x") }),
            ),
        ],
    );
    assert_eq!(answers[0]["result"]["isError"], false, "{answers:?}");
    assert_eq!(answers[1]["result"]["isError"], false, "{answers:?}");
    assert_eq!(answers[2]["result"]["isError"], true);
    let text = answers[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("cannot hand in DEMO-1"), "{text}");
    assert_eq!(tok.status("DEMO-1"), "running");

    // A token the daemon does not know.
    let answers = session(
        &d,
        "not-a-token",
        "DEMO-1",
        &[call(1, "started", json!({}))],
    );
    assert_eq!(answers[0]["result"]["isError"], true);
    assert_eq!(tok.status("DEMO-1"), "running");
}

#[test]
fn tok_mcp_and_tok_hook_outside_an_agent_say_so_or_stay_quiet() {
    let tok = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(args)
            .env_remove("TOKNILA_URL")
            .env_remove("TOKNILA_RUN_TOKEN")
            .env_remove("TOKNILA_TICKET")
            .stdin(Stdio::null())
            .output()
            .expect("run tok")
    };
    let mcp = tok(&["mcp"]);
    assert!(!mcp.status.success());
    assert!(String::from_utf8_lossy(&mcp.stderr).contains("TOKNILA_URL is not set"));
    // A hook in the user's own session must not disturb it.
    let hook = tok(&["hook", "stop"]);
    assert!(hook.status.success());
    assert!(hook.stdout.is_empty() && hook.stderr.is_empty());
}
