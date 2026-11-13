//! The tools of a project's planner (`tok mcp` with the planner role): read the plan, create and
//! edit chains and cards, change dependencies and order, write in threads. A tool call is parsed
//! here into a [`PlannerCall`] and sent to the daemon's `/planner/...` routes with the run's own
//! token; the daemon's use cases decide, so a refused call comes back as the tool's error text.
//! There is no tool that starts, pauses, resumes, cancels, accepts, returns, reopens or merges.

use serde_json::Value;
use toknila_api_types::{
    CardEdit, ChainDetail, EdgeInput, EditPlanChain, NewCard, NewDesignPin, NewMessage,
    NewPlanChain, NewSuggestion, NewThread, NewVersion, PlannerMove, Say, TicketDetail,
};
use toknila_client::Client;
use toknila_core::graph::Place;
use toknila_core::{ArtifactFormat, EdgeKind, NodeRef, ThreadKind};

use super::mcp_tools::ToolError;

mod answer;
mod args;
mod scope;
mod strict;
mod tools;

pub(super) use answer::Answer;
use args::{
    AddCard, Dependency, EditCard, EditChain, Move, OpenThread, ShowCard, ShowDesign,
    ThreadMessage, ThreadVersion,
};
use scope::{PlanScope, plan_scope};
use strict::{args, refuse_unknown_keys};

pub(in crate::cmd) use tools::list;

/// One planner tool call, with its arguments checked.
#[derive(Debug)]
pub(super) enum PlannerCall {
    /// The overview; one chain in full; or one card in full.
    ShowPlan(PlanScope),
    ShowDesign {
        design: String,
    },
    NewChain(NewPlanChain),
    EditChain {
        chain: String,
        edit: EditPlanChain,
    },
    AddCard {
        chain: String,
        card: NewCard,
    },
    EditCard {
        card: String,
        edit: CardEdit,
    },
    Suggest(NewSuggestion),
    Say(Say),
    DesignQuestion(NewDesignPin),
    ShowCard {
        card: String,
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
        anchor: Option<String>,
        place: Option<Place>,
        to_chain: Option<String>,
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

/// The call a tool name and its arguments mean.
pub(super) fn call(name: &str, arguments: &Value) -> Result<PlannerCall, ToolError> {
    let tools = list();
    let Some(tool) = tools.iter().find(|t| t["name"] == name) else {
        return Err(ToolError::Unknown);
    };
    refuse_unknown_keys(name, tool, arguments)?;
    Ok(match name {
        "show_plan" => PlannerCall::ShowPlan(plan_scope(arguments)?),
        "show_design" => {
            let ShowDesign { design } = args(arguments)?;
            PlannerCall::ShowDesign { design }
        }
        "new_chain" => PlannerCall::NewChain(args(arguments)?),
        "edit_chain" => {
            let EditChain { chain, edit } = args(arguments)?;
            PlannerCall::EditChain { chain, edit }
        }
        "add_card" => {
            let AddCard { chain, card } = args(arguments)?;
            PlannerCall::AddCard { chain, card }
        }
        "edit_card" => {
            let EditCard { card, edit } = args(arguments)?;
            PlannerCall::EditCard { card, edit }
        }
        "suggest" => PlannerCall::Suggest(args(arguments)?),
        "say" => PlannerCall::Say(args(arguments)?),
        "design_question" => PlannerCall::DesignQuestion(args(arguments)?),
        "show_card" => {
            let ShowCard { card } = args(arguments)?;
            PlannerCall::ShowCard { card }
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
                to_chain,
            } = args(arguments)?;
            PlannerCall::MoveCard {
                chain,
                card,
                anchor,
                place,
                to_chain,
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
    format!("epic {}: {}", detail.chain.name, labels.join(" < "))
}

fn card_line(verb: &str, d: &TicketDetail) -> String {
    format!(
        "{} {verb} in epic {}: {} ({})",
        d.ticket.key,
        d.chain,
        d.ticket.title,
        toknila_core::wire::wire_name(&d.ticket.status).unwrap_or_default()
    )
}

/// Sends the call to the daemon and answers what came of it: `show_design` also gives the picture.
pub(super) fn send(client: &Client, call: &PlannerCall) -> Result<Answer, String> {
    match call {
        PlannerCall::ShowDesign { design } => answer::show_design(client, design),
        other => send_text(client, other).map(Answer::from),
    }
}

/// Sends the call to the daemon and says in a few lines what came of it.
fn send_text(client: &Client, call: &PlannerCall) -> Result<String, String> {
    let e = |e: toknila_client::ClientError| e.to_string();
    match call {
        PlannerCall::ShowPlan(scope) => {
            // Compact JSON: the overview must stay small at any size of the plan.
            let text = match scope {
                PlanScope::Overview => {
                    serde_json::to_string(&client.planner_overview().map_err(e)?)
                }
                PlanScope::Chain(c) => serde_json::to_string(&client.planner_chain(c).map_err(e)?),
                PlanScope::Card(c) => serde_json::to_string(&client.planner_card(c).map_err(e)?),
            };
            text.map_err(|x| x.to_string())
        }
        PlannerCall::ShowDesign { design } => answer::show_design(client, design).map(|a| a.text),
        PlannerCall::DesignQuestion(q) => {
            let pin = client.planner_pin(q).map_err(e)?;
            Ok(format!(
                "pinned #{} on {}: the owner answers on the mockup, and the answer shows in show_plan under pins",
                pin.number, pin.design_name
            ))
        }
        PlannerCall::NewChain(c) => {
            let d = client.planner_new_chain(c).map_err(e)?;
            Ok(format!(
                "epic {} created; its stories wait for the owner to start them",
                d.chain.name
            ))
        }
        PlannerCall::EditChain { chain, edit } => client
            .planner_edit_chain(chain, edit)
            .map(|d| {
                format!(
                    "epic {} updated (version {})",
                    d.chain.name,
                    d.chain.version.as_deref().unwrap_or("none")
                )
            })
            .map_err(e),
        PlannerCall::AddCard { chain, card } => client
            .planner_add_card(chain, card)
            .map(|d| card_line("added", &d))
            .map_err(e),
        PlannerCall::EditCard { card, edit } => client
            .planner_edit_card(card, edit)
            .map(|d| card_line("updated", &d))
            .map_err(e),
        PlannerCall::Suggest(s) => client
            .planner_suggest(s)
            .map(|s| match s.card {
                Some(card) => format!(
                    "next step kept: {} {card}",
                    s.action.map_or("", |a| a.as_str())
                ),
                None => "next step kept".to_owned(),
            })
            .map_err(e),
        PlannerCall::Say(s) => client
            .planner_say(s)
            .map(|_| "said: the owner reads it in the chat".to_owned())
            .map_err(e),
        PlannerCall::ShowCard { card } => client
            .planner_show_card(card)
            .map(|d| {
                format!(
                    "{} shown in the window, if it looks at this project",
                    d.ticket.key
                )
            })
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
            to_chain,
        } => {
            let m = PlannerMove {
                anchor: anchor.clone(),
                place: *place,
                to_chain: to_chain.clone(),
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
    use serde_json::json;

    use super::*;

    fn names() -> Vec<String> {
        list()
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn the_planner_has_its_sixteen_tools_and_none_that_starts_or_accepts_anything() {
        let names = names();
        assert_eq!(names.len(), 16, "{names:?}");
        assert!(names.iter().any(|n| n == "say"));
        assert!(matches!(
            call("say", &json!({ "text": "Hi", "about": "DEMO-2" })),
            Ok(PlannerCall::Say(s)) if s.text == "Hi" && s.about.as_deref() == Some("DEMO-2")
        ));
        assert!(matches!(
            call("say", &json!({})),
            Err(ToolError::BadArguments(_))
        ));
        assert!(call("say", &json!({ "text": "x", "colour": "red" })).is_err());
        assert!(names.iter().any(|n| n == "edit_chain"));
        assert!(names.iter().any(|n| n == "design_question"));
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
        assert!(matches!(
            c("show_plan", json!({})),
            PlannerCall::ShowPlan(PlanScope::Overview)
        ));
        assert!(matches!(
            c("show_plan", json!({ "chain": "phase-6" })),
            PlannerCall::ShowPlan(PlanScope::Chain(n)) if n == "phase-6"
        ));
        assert!(matches!(
            c("show_plan", json!({ "card": "DEMO-2" })),
            PlannerCall::ShowPlan(PlanScope::Card(n)) if n == "DEMO-2"
        ));
        assert!(call("show_plan", &json!({ "chain": "a", "card": "b" })).is_err());
        assert!(matches!(
            c("show_design", json!({ "design": "Home" })),
            PlannerCall::ShowDesign { design } if design == "Home"
        ));
        assert!(matches!(
            call("show_design", &json!({})),
            Err(ToolError::BadArguments(_))
        ));
        let PlannerCall::DesignQuestion(q) = c(
            "design_question",
            json!({ "design": "Home", "region": { "x": 1, "y": 2, "w": 3, "h": 4 },
                    "text": "Tax?", "card": "DEMO-1" }),
        ) else {
            panic!("design_question is a DesignQuestion");
        };
        assert_eq!((q.design.as_str(), q.text.as_str()), ("Home", "Tax?"));
        assert_eq!((q.region.w, q.card.as_deref()), (3, Some("DEMO-1")));
        assert!(matches!(
            call("design_question", &json!({ "design": "Home" })),
            Err(ToolError::BadArguments(_))
        ));
        assert!(matches!(
            c("new_chain", json!({ "name": "plan", "goal": "g" })),
            PlannerCall::NewChain(n) if n.name.as_deref() == Some("plan") && n.goal.as_deref() == Some("g")
        ));
        assert!(matches!(
            c("new_chain", json!({ "title": "MIDI controllers" })),
            PlannerCall::NewChain(n) if n.name.is_none() && n.title.as_deref() == Some("MIDI controllers")
        ));
        assert!(matches!(
            c("new_chain", json!({ "name": "plan", "version": "0.4.0" })),
            PlannerCall::NewChain(n) if n.version.as_deref() == Some("0.4.0")
        ));
        assert!(matches!(
            c("edit_chain", json!({ "chain": "plan", "version": "0.5.0", "goal": "g" })),
            PlannerCall::EditChain { chain, edit }
                if chain == "plan" && edit.version.as_deref() == Some("0.5.0") && edit.name.is_none()
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
                    "designs": [{ "design": "login.png", "region": { "x": 1, "y": 2, "w": 3, "h": 4 } }, { "design": "home.png" }],
                    "edit_subtasks": [{ "position": 1, "text": "y" }] }),
        ) else {
            panic!("edit_card")
        };
        let picks = edit.designs.as_ref().expect("designs");
        assert_eq!(
            (
                picks.len(),
                picks[0].design.as_str(),
                picks[0].region.map(|r| r.w)
            ),
            (2, "login.png", Some(3))
        );
        assert!(picks[1].region.is_none());
        assert_eq!(card, "DEMO-2");
        assert_eq!(edit.title.as_deref(), Some("N"));
        assert_eq!(edit.edit_subtasks[0].position, 1);
        assert!(matches!(
            c("show_card", json!({ "card": "DEMO-2" })),
            PlannerCall::ShowCard { card } if card == "DEMO-2"
        ));
        assert!(matches!(
            call("show_card", &json!({})),
            Err(ToolError::BadArguments(_))
        ));
        match c(
            "suggest",
            json!({ "text": "Start A", "card": "DEMO-1", "action": "start" }),
        ) {
            PlannerCall::Suggest(s) => {
                assert_eq!(s.text, "Start A");
                assert_eq!(s.card.as_deref(), Some("DEMO-1"));
                assert_eq!(
                    s.action,
                    Some(toknila_core::suggestion::SuggestionAction::Start)
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            call("suggest", &json!({ "text": "x", "action": "fly" })),
            Err(ToolError::BadArguments(_))
        ));
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
                place: Some(Place::Before),
                to_chain: None,
                ..
            }
        ));
        assert!(matches!(
            c(
                "move_card",
                json!({ "chain": "p", "card": "A-2", "to_chain": "q" })
            ),
            PlannerCall::MoveCard {
                anchor: None,
                place: None,
                to_chain: Some(ref to),
                ..
            } if to == "q"
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
