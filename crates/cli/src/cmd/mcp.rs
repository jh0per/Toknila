//! `tok mcp`: the agent event contract as an MCP server on stdio. The agent's host starts it
//! (the runner wires it in); it forwards every tool call to the daemon's agent event route with
//! the run's own token, the same route `tok sim` uses.
//!
//! Minimal JSON-RPC 2.0 over newline-delimited JSON: `initialize`, `ping`, `tools/list` and
//! `tools/call`; notifications get no answer. Nothing but protocol messages goes to stdout.

use std::io::{BufRead, Write};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use toknila_api_types::{AgentEvent, TicketDetail};
use toknila_core::AgentRole;

use super::agent::AgentEnv;
use super::mcp_tools::{self, ToolError};

/// The protocol revision answered when the client names none.
const PROTOCOL: &str = "2025-06-18";

const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Forwards an event to the daemon and says in a line how the ticket stands, or why not.
type Forward<'a> = dyn Fn(&AgentEvent) -> Result<String, String> + 'a;

fn reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool's result; `is_error` marks a refused call (the agent reads the text and adapts).
fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn call_tool(
    role: AgentRole,
    params: &Value,
    forward: &Forward<'_>,
) -> Result<Value, (i64, String)> {
    let name = params["name"].as_str().unwrap_or_default();
    match mcp_tools::event(role, name, &params["arguments"]) {
        Err(ToolError::Unknown) => Err((INVALID_PARAMS, format!("unknown tool {name:?}"))),
        Err(ToolError::BadArguments(e)) => Ok(tool_result(&format!("{name}: {e}"), true)),
        Ok(event) => Ok(match forward(&event) {
            Ok(line) => tool_result(&line, false),
            Err(e) => tool_result(&e, true),
        }),
    }
}

/// The answer to one message, or none for a notification.
fn handle(role: AgentRole, message: &Value, forward: &Forward<'_>) -> Option<Value> {
    let id = message.get("id")?;
    let params = &message["params"];
    let outcome = match message["method"].as_str().unwrap_or_default() {
        "initialize" => Ok(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "toknila", "version": toknila_core::VERSION },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": mcp_tools::list(role) })),
        "tools/call" => call_tool(role, params, forward),
        other => Err((
            METHOD_NOT_FOUND,
            format!("method {other:?} is not supported"),
        )),
    };
    Some(match outcome {
        Ok(result) => reply(id, result),
        Err((code, message)) => failure(id, code, &message),
    })
}

/// Reads messages from `input` until it ends and writes the answers to `output`.
fn serve(
    role: AgentRole,
    input: impl BufRead,
    mut output: impl Write,
    forward: &Forward<'_>,
) -> Result<()> {
    for line in input.lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(role, &message, forward),
            Err(e) => Some(failure(
                &Value::Null,
                PARSE_ERROR,
                &format!("not JSON: {e}"),
            )),
        };
        if let Some(answer) = answer {
            writeln!(output, "{answer}").context("writing stdout")?;
            output.flush().context("writing stdout")?;
        }
    }
    Ok(())
}

/// One line on the ticket after an event: `DEMO-1 waiting`.
fn stands(detail: &TicketDetail) -> String {
    let status = serde_json::to_value(detail.ticket.status)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("{} is {status}", detail.ticket.key)
}

pub(super) fn run() -> Result<()> {
    let agent = AgentEnv::from_env().map_err(anyhow::Error::msg)?;
    let forward = |event: &AgentEvent| {
        agent
            .client
            .agent_event(&agent.ticket, event)
            .map(|d| stands(&d))
            .map_err(|e| e.to_string())
    };
    serve(
        agent.role,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &forward,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exchange(lines: &[Value], forward: &Forward<'_>) -> Vec<Value> {
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        serve(AgentRole::Worker, input.as_bytes(), &mut out, forward).expect("serve");
        String::from_utf8(out)
            .expect("utf-8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("json line"))
            .collect()
    }

    fn ok(_: &AgentEvent) -> Result<String, String> {
        Ok("DEMO-1 is running".to_owned())
    }

    #[test]
    fn the_handshake_lists_the_tools_and_notifications_get_no_answer() {
        let answers = exchange(
            &[
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
                json!({"jsonrpc":"2.0","id":3,"method":"ping"}),
            ],
            &ok,
        );
        assert_eq!(answers.len(), 3, "{answers:?}");
        assert_eq!(answers[0]["id"], 1);
        assert_eq!(answers[0]["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(answers[0]["result"]["serverInfo"]["name"], "toknila");
        assert!(answers[0]["result"]["capabilities"]["tools"].is_object());
        let tools = answers[1]["result"]["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 10);
        assert_eq!(answers[2]["result"], json!({}));
    }

    #[test]
    fn a_tool_call_becomes_the_event_and_the_daemon_answer_comes_back() {
        let seen = std::cell::RefCell::new(Vec::new());
        let forward = |e: &AgentEvent| {
            seen.borrow_mut().push(e.clone());
            Ok("DEMO-1 is waiting".to_owned())
        };
        let answers = exchange(
            &[json!({"jsonrpc":"2.0","id":7,"method":"tools/call",
                "params":{"name":"ask_human","arguments":{"text":"which?","permission":true}}})],
            &forward,
        );
        assert_eq!(
            *seen.borrow(),
            [AgentEvent::AskHuman {
                text: "which?".into(),
                permission: true
            }]
        );
        assert_eq!(answers[0]["id"], 7);
        assert_eq!(answers[0]["result"]["isError"], false);
        assert_eq!(
            answers[0]["result"]["content"][0]["text"],
            "DEMO-1 is waiting"
        );
    }

    #[test]
    fn a_refused_event_is_a_tool_error_and_protocol_errors_are_json_rpc_errors() {
        let refuse = |_: &AgentEvent| Err("cannot hand in DEMO-1: 1 open".to_owned());
        let answers = exchange(
            &[
                json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                    "params":{"name":"finished","arguments":{"summary":"x"}}}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
                    "params":{"name":"finished","arguments":{}}}),
                json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"approve"}}),
                json!({"jsonrpc":"2.0","id":4,"method":"resources/list"}),
            ],
            &refuse,
        );
        assert_eq!(answers[0]["result"]["isError"], true);
        assert_eq!(
            answers[0]["result"]["content"][0]["text"],
            "cannot hand in DEMO-1: 1 open"
        );
        assert_eq!(answers[1]["result"]["isError"], true);
        assert_eq!(answers[2]["error"]["code"], INVALID_PARAMS);
        assert_eq!(answers[3]["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn garbage_gets_a_parse_error_and_the_server_keeps_going() {
        let mut out = Vec::new();
        let input = "not json\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
        serve(AgentRole::Worker, input.as_bytes(), &mut out, &ok).expect("serve");
        let lines: Vec<Value> = String::from_utf8(out)
            .expect("utf-8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("json"))
            .collect();
        assert_eq!(lines[0]["error"]["code"], PARSE_ERROR);
        assert_eq!(lines[1]["result"], json!({}));
    }
}
