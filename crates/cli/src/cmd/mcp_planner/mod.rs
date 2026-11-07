//! The tools of a project's planner (`tok mcp` with the planner role): read the plan, create and
//! edit chains and cards, change dependencies and order, write in threads. A tool call is parsed
//! here into a [`PlannerCall`] and sent to the daemon's `/planner/...` routes with the run's own
//! token; the daemon's use cases decide, so a refused call comes back as the tool's error text.
//! There is no tool that starts, pauses, resumes, cancels, accepts, returns, reopens or merges.

use serde::Deserialize;
use serde_json::{Value, json};
use toknila_api_types::{
    CardEdit, ChainDetail, EdgeInput, MoveInput, NewCard, NewMessage, NewPlanChain, NewThread,
    NewVersion, TicketDetail,
};
use toknila_client::Client;
use toknila_core::graph::Place;
use toknila_core::{ArtifactFormat, EdgeKind, NodeRef, ThreadKind};

use super::mcp_tools::ToolError;

mod tools;

pub(in crate::cmd) use tools::list;

/// One planner tool call, with its arguments checked.
#[derive(Debug)]
pub(super) enum PlannerCall {
    ShowPlan,
    NewChain(NewPlanChain),
    AddCard {
        chain: String,
        card: NewCard,
    },
    EditCard {
        card: String,
        edit: CardEdit,
    },
    AddDependency {
        chain: String,
        from: String,
        to: String,
    },
    RemoveDependency {
        chain: String,
        from: String,
        to: String,
    },
    MoveCard {
        chain: String,
        card: String,
        anchor: String,
        place: Place,
    },
    OpenThread {
        card: String,
        kind: ThreadKind,
        title: String,
    },
    ThreadMessage {
        card: String,
        thread: String,
        text: String,
    },
    ThreadVersion {
        card: String,
        thread: String,
        format: ArtifactFormat,
        body: String,
    },
}

#[derive(Deserialize)]
struct AddCard {
    chain: String,
    #[serde(flatten)]
    card: NewCard,
}

#[derive(Deserialize)]
struct EditCard {
    card: String,
    #[serde(flatten)]
    edit: CardEdit,
}

#[derive(Deserialize)]
struct Dependency {
    chain: String,
    from: String,
    to: String,
}

#[derive(Deserialize)]
struct Move {
    chain: String,
    card: String,
    anchor: String,
    place: Place,
}

#[derive(Deserialize)]
struct OpenThread {
    card: String,
    kind: ThreadKind,
    title: String,
}

#[derive(Deserialize)]
struct ThreadMessage {
    card: String,
    thread: String,
    text: String,
}

#[derive(Deserialize)]
struct ThreadVersion {
    card: String,
    thread: String,
    format: ArtifactFormat,
    body: String,
}

fn args<T: serde::de::DeserializeOwned>(arguments: &Value) -> Result<T, ToolError> {
    let value = match arguments {
        Value::Null => json!({}),
        other => other.clone(),
    };
    serde_json::from_value(value).map_err(|e| ToolError::BadArguments(e.to_string()))
}

/// The call a tool name and its arguments mean.
pub(super) fn call(name: &str, arguments: &Value) -> Result<PlannerCall, ToolError> {
    if !list().iter().any(|t| t["name"] == name) {
        return Err(ToolError::Unknown);
    }
    Ok(match name {
        "show_plan" => PlannerCall::ShowPlan,
        "new_chain" => PlannerCall::NewChain(args(arguments)?),
        "add_card" => {
            let AddCard { chain, card } = args(arguments)?;
            PlannerCall::AddCard { chain, card }
        }
        "edit_card" => {
            let EditCard { card, edit } = args(arguments)?;
            PlannerCall::EditCard { card, edit }
        }
        "add_dependency" => {
            let Dependency { chain, from, to } = args(arguments)?;
            PlannerCall::AddDependency { chain, from, to }
        }
        "remove_dependency" => {
            let Dependency { chain, from, to } = args(arguments)?;
            PlannerCall::RemoveDependency { chain, from, to }
        }
        "move_card" => {
            let Move {
                chain,
                card,
                anchor,
                place,
            } = args(arguments)?;
            PlannerCall::MoveCard {
                chain,
                card,
                anchor,
                place,
            }
        }
        "open_thread" => {
            let OpenThread { card, kind, title } = args(arguments)?;
            PlannerCall::OpenThread { card, kind, title }
        }
        "thread_message" => {
            let ThreadMessage { card, thread, text } = args(arguments)?;
            PlannerCall::ThreadMessage { card, thread, text }
        }
        "thread_version" => {
            let ThreadVersion {
                card,
                thread,
                format,
                body,
            } = args(arguments)?;
            PlannerCall::ThreadVersion {
                card,
                thread,
                format,
                body,
            }
        }
        _ => return Err(ToolError::Unknown),
    })
}

/// The chain's cards in order, one line: `DEMO-1 < DEMO-3 < DEMO-2`.
fn order(detail: &ChainDetail) -> String {
    let labels: Vec<String> = detail
        .order
        .iter()
        .filter_map(|n| match n {
            NodeRef::Ticket(id) => detail
                .tickets
                .iter()
                .find(|t| t.id == *id)
                .map(|t| t.key.to_string()),
            NodeRef::Release(_) => None,
        })
        .collect();
    format!("chain {}: {}", detail.chain.name, labels.join(" < "))
}

fn card_line(verb: &str, d: &TicketDetail) -> String {
    format!(
        "{} {verb} in chain {}: {} ({})",
        d.ticket.key,
        d.chain,
        d.ticket.title,
        toknila_core::wire::wire_name(&d.ticket.status).unwrap_or_default()
    )
}

/// Sends the call to the daemon and says in a few lines what came of it.
pub(super) fn send(client: &Client, call: &PlannerCall) -> Result<String, String> {
    let e = |e: toknila_client::ClientError| e.to_string();
    match call {
        PlannerCall::ShowPlan => {
            let plan = client.planner_plan().map_err(e)?;
            serde_json::to_string_pretty(&plan).map_err(|x| x.to_string())
        }
        PlannerCall::NewChain(c) => {
            let d = client.planner_new_chain(c).map_err(e)?;
            Ok(format!(
                "chain {} created; its cards wait for the owner to start them",
                d.chain.name
            ))
        }
        PlannerCall::AddCard { chain, card } => client
            .planner_add_card(chain, card)
            .map(|d| card_line("added", &d))
            .map_err(e),
        PlannerCall::EditCard { card, edit } => client
            .planner_edit_card(card, edit)
            .map(|d| card_line("updated", &d))
            .map_err(e),
        PlannerCall::AddDependency { chain, from, to } => {
            let edge = EdgeInput {
                from: from.clone(),
                to: to.clone(),
                kind: EdgeKind::Normal,
            };
            client
                .planner_add_dependency(chain, &edge)
                .map(|d| order(&d))
                .map_err(e)
        }
        PlannerCall::RemoveDependency { chain, from, to } => client
            .planner_remove_dependency(chain, from, to)
            .map(|d| order(&d))
            .map_err(e),
        PlannerCall::MoveCard {
            chain,
            card,
            anchor,
            place,
        } => {
            let m = MoveInput {
                anchor: anchor.clone(),
                place: *place,
            };
            client
                .planner_move_card(chain, card, &m)
                .map(|d| order(&d))
                .map_err(e)
        }
        PlannerCall::OpenThread { card, kind, title } => {
            let t = NewThread {
                kind: *kind,
                title: title.clone(),
            };
            client
                .planner_open_thread(card, &t)
                .map(|d| {
                    format!(
                        "thread #{} opened on {card}: {}",
                        d.thread.number, d.thread.title
                    )
                })
                .map_err(e)
        }
        PlannerCall::ThreadMessage { card, thread, text } => {
            let m = NewMessage { text: text.clone() };
            client
                .planner_thread_message(card, thread, &m)
                .map(|d| format!("posted in thread #{} of {card}", d.thread.number))
                .map_err(e)
        }
        PlannerCall::ThreadVersion {
            card,
            thread,
            format,
            body,
        } => {
            let v = NewVersion {
                format: *format,
                body: body.clone(),
                message: None,
            };
            client
                .planner_thread_version(card, thread, &v)
                .map(|d| {
                    format!(
                        "version {} posted in thread #{} of {card}",
                        d.versions.len(),
                        d.thread.number
                    )
                })
                .map_err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        list()
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn the_planner_has_its_ten_tools_and_none_that_starts_or_accepts_anything() {
        let names = names();
        assert_eq!(names.len(), 10, "{names:?}");
        for forbidden in [
            "start", "pause", "resume", "cancel", "accept", "return", "reopen", "merge", "approve",
            "unblock", "block", "finished", "started",
        ] {
            assert!(
                !names
                    .iter()
                    .any(|n| n == forbidden || n.starts_with(&format!("{forbidden}_"))),
                "{forbidden}"
            );
            assert!(
                matches!(call(forbidden, &json!({})), Err(ToolError::Unknown)),
                "{forbidden} is not a planner tool"
            );
        }
    }

    #[test]
    fn every_tool_turns_its_arguments_into_its_call() {
        let c = |name, args| call(name, &args).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(matches!(c("show_plan", json!({})), PlannerCall::ShowPlan));
        assert!(matches!(
            c("new_chain", json!({ "name": "plan", "goal": "g" })),
            PlannerCall::NewChain(n) if n.name == "plan" && n.goal.as_deref() == Some("g")
        ));
        let PlannerCall::AddCard { chain, card } = c(
            "add_card",
            json!({ "chain": "plan", "title": "T", "prompt": "p", "acceptance": ["a"],
                    "after": ["DEMO-1"], "subtasks": ["s"] }),
        ) else {
            panic!("add_card")
        };
        assert_eq!(chain, "plan");
        assert_eq!(
            (
                card.title.as_str(),
                card.after.clone(),
                card.subtasks.clone()
            ),
            ("T", vec!["DEMO-1".to_owned()], vec!["s".to_owned()])
        );
        let PlannerCall::EditCard { card, edit } = c(
            "edit_card",
            json!({ "card": "DEMO-2", "title": "N", "add_subtasks": ["x"],
                    "edit_subtasks": [{ "position": 1, "text": "y" }] }),
        ) else {
            panic!("edit_card")
        };
        assert_eq!(card, "DEMO-2");
        assert_eq!(edit.title.as_deref(), Some("N"));
        assert_eq!(edit.edit_subtasks[0].position, 1);
        assert!(matches!(
            c(
                "add_dependency",
                json!({ "chain": "p", "from": "A-1", "to": "A-2" })
            ),
            PlannerCall::AddDependency { .. }
        ));
        assert!(matches!(
            c(
                "remove_dependency",
                json!({ "chain": "p", "from": "A-1", "to": "A-2" })
            ),
            PlannerCall::RemoveDependency { .. }
        ));
        assert!(matches!(
            c(
                "move_card",
                json!({ "chain": "p", "card": "A-2", "anchor": "A-1", "place": "before" })
            ),
            PlannerCall::MoveCard {
                place: Place::Before,
                ..
            }
        ));
        assert!(matches!(
            c(
                "open_thread",
                json!({ "card": "A-1", "kind": "flow", "title": "t" })
            ),
            PlannerCall::OpenThread {
                kind: ThreadKind::Flow,
                ..
            }
        ));
        assert!(matches!(
            c(
                "thread_message",
                json!({ "card": "A-1", "thread": "1", "text": "hi" })
            ),
            PlannerCall::ThreadMessage { .. }
        ));
        assert!(matches!(
            c(
                "thread_version",
                json!({ "card": "A-1", "thread": "1", "format": "mermaid", "body": "graph TD" })
            ),
            PlannerCall::ThreadVersion {
                format: ArtifactFormat::Mermaid,
                ..
            }
        ));
    }

    #[test]
    fn bad_arguments_are_told_apart_from_unknown_tools() {
        assert!(matches!(
            call("add_card", &json!({ "title": "no chain" })),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            call(
                "move_card",
                &json!({ "chain": "p", "card": "a", "anchor": "b", "place": "sideways" })
            ),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            call("started", &json!({})),
            Err(ToolError::Unknown)
        ));
    }
}
