//! The tools of `tok mcp`: the agent event contract (`docs/events.md`), one tool per event. A
//! tool is named like its event and takes the event's fields, so a call is turned into an event
//! by serde alone and goes through the same daemon route as `tok sim`.

use serde_json::{Map, Value, json};
use toknila_api_types::AgentEvent;
use toknila_core::AgentRole;

/// Why a tool call is not an event.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ToolError {
    /// No tool has this name.
    Unknown,
    /// The arguments do not fit the tool; the text says how.
    BadArguments(String),
}

pub(super) fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
    })
}

pub(super) fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

pub(super) fn list_of_text(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": description })
}

/// What `tools/list` answers: a worker's tools report its work; a reviewer has one, its verdict.
pub(super) fn list(role: AgentRole) -> Vec<Value> {
    match role {
        AgentRole::Reviewer => vec![review_verdict()],
        AgentRole::Planner => super::mcp_planner::list(),
        _ => worker_tools(),
    }
}

fn review_verdict() -> Value {
    tool(
        "review_verdict",
        "Give your verdict on the finished work and end your review. Judge every acceptance item PASS or FAIL in `items`, then approve (only when every item passes) or request changes with a comment saying exactly what to change. Call it once, when you have read the diff.",
        json!({
            "verdict": { "type": "string", "enum": ["approve", "request_changes"] },
            "items": {
                "type": "array",
                "description": "One entry per acceptance item of the ticket, by its 1-based position; required when the ticket has acceptance items",
                "items": {
                    "type": "object",
                    "properties": {
                        "item": { "type": "integer", "minimum": 1 },
                        "outcome": { "type": "string", "enum": ["pass", "fail", "person"], "description": "person: only a person can judge it (a picture, a look, a feeling, an approval the record does not hold); it does not count against the hand-in" },
                        "basis": {
                            "type": "string",
                            "enum": ["ran", "read", "not_run"],
                            "description": "ran: you ran the check or test that shows it and quoted its result; read: you judged it from the diff and files; not_run: the check or test it names could not be run",
                        },
                        "note": text("One line of evidence; quote the result line of what you ran"),
                    },
                    "required": ["item", "outcome"],
                },
            },
            "comment": text("What to change (required with request_changes unless an item fails); optional remarks otherwise"),
        }),
        &["verdict"],
    )
}

fn worker_tools() -> Vec<Value> {
    let thread = text("The thread's number on this ticket (1, 2, ...), or its id");
    vec![
        tool(
            "started",
            "Report that you have started working on the ticket.",
            json!({}),
            &[],
        ),
        tool(
            "ask_human",
            "Ask the person a question, or ask for permission, and wait. The ticket goes to Waiting until they answer.",
            json!({
                "text": text("The question, or the action you need permission for"),
                "permission": { "type": "boolean", "description": "True when asking permission for an action rather than a question" },
                "choices": { "type": "array", "items": { "type": "string" }, "minItems": 2, "maxItems": 4,
                    "description": "Two to four short sentences the person can act on, the recommended one first; the person presses one and you get its text back" },
            }),
            &["text"],
        ),
        tool(
            "subtask_done",
            "Tick an item of the ticket's checklist.",
            json!({ "position": { "type": "integer", "minimum": 1, "description": "1-based position of the item" } }),
            &["position"],
        ),
        tool(
            "note",
            "Leave a short progress report on the ticket.",
            json!({ "body": { "type": "object", "description": "The report",
                "properties": {
                    "gist": text("One line"),
                    "what": text("What was done"),
                    "why": text("Why"),
                    "how_to_verify": text("How to check it"),
                    "alternatives": text("What was rejected and why"),
                    "consequences": text("What changes next, and risks"),
                },
                "required": ["gist"],
            }}),
            &["body"],
        ),
        tool(
            "finished",
            "Hand the ticket in for review, with the hand-in template as the summary. Refused while a thread is still open or when the summary has no checks section. Do not call it after an ordinary reply: only when the work is done.",
            json!({ "summary": text("**What changed:** ...\n**Checks:**\n- PASS|FAIL|NOT_RUN|BLOCKED <check>: <evidence>\n**Left:** ...") }),
            &["summary"],
        ),
        tool(
            "crashed",
            "Report that you cannot continue (an unrecoverable failure). The ticket is blocked until a person unblocks it.",
            json!({ "detail": text("What failed") }),
            &["detail"],
        ),
        tool(
            "propose",
            "Propose work nobody planned as a new card. It waits for the person's approval and changes nothing else.",
            json!({
                "title": text("Short title of the card"),
                "prompt": text("The task for whoever does it"),
                "acceptance": list_of_text("Acceptance criteria"),
                "after": list_of_text("Tickets (keys or titles) or release versions the card waits for"),
                "before": list_of_text("Tickets that should wait for the card; none may have started"),
            }),
            &["title"],
        ),
        tool(
            "open_thread",
            "Open a thread on this ticket to settle a design, a flow or an architecture with the person before implementing. The hand-in is refused until every thread that has a version is approved. A card gets one thread of each kind: a second open_thread of the same kind is refused with the number of the existing thread; post a version on it with thread_version instead.",
            json!({
                "kind": { "type": "string", "enum": ["design", "flow", "architecture", "other"] },
                "title": text("What the thread is about"),
            }),
            &["kind", "title"],
        ),
        tool(
            "thread_message",
            "Post a message in a thread of this ticket.",
            json!({ "thread": thread, "text": text("The message") }),
            &["thread", "text"],
        ),
        tool(
            "thread_version",
            "Post the next version of a thread's artifact (a diagram, a screen description). A new version reopens an approved thread.",
            json!({
                "thread": text("The thread's number on this ticket, or its id"),
                "format": { "type": "string", "enum": ["mermaid", "markdown", "html"] },
                "body": text("The artifact, at most 512 KiB"),
            }),
            &["thread", "format", "body"],
        ),
        tool(
            "withdraw_thread",
            "Withdraw a thread you opened by mistake, while it has no version and no person message. A thread with no version never blocks the hand-in.",
            json!({ "thread": thread }),
            &["thread"],
        ),
    ]
}

/// The event a tool call means: the tool's name is the event's tag, its arguments the fields.
pub(super) fn event(
    role: AgentRole,
    name: &str,
    arguments: &Value,
) -> Result<AgentEvent, ToolError> {
    let known = list(role).iter().any(|t| t["name"] == name);
    if !known {
        return Err(ToolError::Unknown);
    }
    let mut fields: Map<String, Value> = match arguments {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => {
            return Err(ToolError::BadArguments(
                "arguments must be an object".into(),
            ));
        }
    };
    fields.insert("event".to_owned(), Value::String(name.to_owned()));
    serde_json::from_value(Value::Object(fields))
        .map_err(|e| ToolError::BadArguments(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_one_tool_for_every_agent_event_and_each_maps_to_it() {
        let cases = [
            ("started", json!({})),
            ("ask_human", json!({ "text": "which?" })),
            ("subtask_done", json!({ "position": 2 })),
            ("note", json!({ "body": { "gist": "half way" } })),
            ("finished", json!({ "summary": "done" })),
            ("crashed", json!({ "detail": "exit 101" })),
            ("propose", json!({ "title": "t", "before": ["DEMO-2"] })),
            ("open_thread", json!({ "kind": "design", "title": "List" })),
            ("thread_message", json!({ "thread": "1", "text": "hi" })),
            (
                "thread_version",
                json!({ "thread": "1", "format": "mermaid", "body": "graph TD" }),
            ),
            ("withdraw_thread", json!({ "thread": "2" })),
        ];
        let names: Vec<_> = list(AgentRole::Worker)
            .iter()
            .map(|t| t["name"].clone())
            .collect();
        assert_eq!(names.len(), cases.len());
        for (name, args) in cases {
            assert!(names.contains(&json!(name)), "{name} is listed");
            let e =
                event(AgentRole::Worker, name, &args).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let wire = serde_json::to_value(&e).expect("json");
            assert_eq!(wire["event"], name);
            for (k, v) in args.as_object().expect("object") {
                assert_eq!(&wire[k], v, "{name}.{k}");
            }
        }
    }

    #[test]
    fn a_reviewer_has_its_verdict_and_nothing_else_and_a_worker_cannot_give_it() {
        let reviewer = list(AgentRole::Reviewer);
        let names: Vec<_> = reviewer.iter().map(|t| t["name"].clone()).collect();
        assert_eq!(names, [json!("review_verdict")]);
        let args = json!({ "verdict": "request_changes", "comment": "rename it" });
        let e = event(AgentRole::Reviewer, "review_verdict", &args).expect("event");
        assert_eq!(
            serde_json::to_value(&e).expect("json"),
            json!({ "event": "review_verdict", "verdict": "request_changes", "comment": "rename it" })
        );
        assert_eq!(
            event(AgentRole::Reviewer, "finished", &json!({ "summary": "x" })),
            Err(ToolError::Unknown)
        );
        assert_eq!(
            event(AgentRole::Worker, "review_verdict", &args),
            Err(ToolError::Unknown)
        );
        assert!(matches!(
            event(
                AgentRole::Reviewer,
                "review_verdict",
                &json!({ "verdict": "maybe" })
            ),
            Err(ToolError::BadArguments(_))
        ));
    }

    #[test]
    fn unknown_tools_and_bad_arguments_are_told_apart() {
        let worker = |name: &str, args: Value| event(AgentRole::Worker, name, &args);
        assert_eq!(worker("approve", json!({})), Err(ToolError::Unknown));
        assert_eq!(worker("hook", json!({})), Err(ToolError::Unknown));
        assert!(matches!(
            worker("finished", json!({})),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            worker("subtask_done", json!({ "position": "one" })),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            worker("started", json!([1])),
            Err(ToolError::BadArguments(_))
        ));
        // The `event` field of the arguments cannot pick another event than the tool's name.
        let e = worker("started", json!({ "event": "finished", "summary": "x" })).expect("event");
        assert_eq!(e, AgentEvent::Started);
    }
}
