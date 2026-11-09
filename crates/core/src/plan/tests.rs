use std::collections::HashMap;

use super::*;
use crate::graph::ChainView;
use crate::{ProjectId, TicketKey, Timestamp};

const DEMO: &str = include_str!("../../../../examples/demo.toml");

fn keys() -> impl FnMut() -> TicketKey {
    let mut n = 0;
    move || {
        n += 1;
        TicketKey::new("DEMO".parse().expect("project key"), n).expect("ticket key")
    }
}

/// Graph shape by titles and versions, independent of ids and file order.
fn shape(view: &ChainView) -> Vec<(String, String, EdgeKind)> {
    let mut e: Vec<_> = view
        .edges
        .iter()
        .map(|e| (view.label(e.from), view.label(e.to), e.kind))
        .collect();
    e.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    e
}

fn texts(lists: Vec<Checklist>) -> HashMap<crate::TicketId, Vec<String>> {
    let text = |c: Checklist| c.subtasks.into_iter().map(|s| s.0).collect();
    lists.into_iter().map(|c| (c.ticket, text(c))).collect()
}

fn err(text: &str) -> PlanError {
    parse(text).expect_err("should be rejected")
}

#[test]
fn demo_parses_into_the_gate_graph() {
    let plan = parse(DEMO).expect("demo parses");
    assert_eq!(plan.chain.name, "demo");
    assert!(plan.chain.autostart);
    let titles: Vec<_> = plan.tickets.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["A", "B", "D", "C"]);
    assert_eq!(plan.releases[0].version, "R1");
    assert_eq!(plan.releases[0].start_mode, StartMode::Manual);
    assert_eq!(plan.tickets[0].subtasks, ["write the test", "make it pass"]);
    let (view, _) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    view.validate().expect("valid graph");
    let names: Vec<_> = shape(&view)
        .into_iter()
        .map(|(a, b, _)| format!("{a}>{b}"))
        .collect();
    assert_eq!(
        names,
        [
            "DEMO-1>DEMO-2",
            "DEMO-1>DEMO-3",
            "DEMO-2>release R1",
            "DEMO-3>release R1",
            "release R1>DEMO-4"
        ]
    );
}

#[test]
fn export_then_import_gives_the_same_graph() {
    let plan = parse(DEMO).expect("demo parses");
    let (view, subtasks) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    let subtasks = texts(subtasks);
    let text = export(&view, &subtasks);
    let again = parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    let (view2, subtasks2) = again.instantiate(ProjectId::new(), Timestamp(0), keys());

    let tickets = |v: &ChainView| {
        let mut t: Vec<_> = v
            .tickets
            .iter()
            .map(|t| {
                (
                    t.key.to_string(),
                    t.title.clone(),
                    t.prompt.clone(),
                    t.acceptance.clone(),
                    t.profile.clone(),
                )
            })
            .collect();
        t.sort();
        t
    };
    assert_eq!(tickets(&view), tickets(&view2));
    assert_eq!(shape(&view), shape(&view2));
    assert_eq!(view.releases[0].version, view2.releases[0].version);
    assert_eq!(view.releases[0].start_mode, view2.releases[0].start_mode);
    let st2 = texts(subtasks2);
    let a2 = view2.tickets.iter().find(|t| t.title == "A").expect("A");
    assert_eq!(st2[&a2.id], ["write the test", "make it pass"]);
    // A second export is byte-identical: the format is stable.
    assert_eq!(text, export(&view2, &st2));
}

#[test]
fn cards_outside_the_plan_are_not_exported() {
    let plan = parse(DEMO).expect("demo parses");
    let (mut view, _) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    let mark = crate::Wildcard {
        from: None,
        proposed_by: crate::Actor::Agent,
        after: vec![],
        before: vec![],
        approved: false,
    };
    let draft = crate::TicketDraft {
        title: "token refresh".into(),
        ..crate::TicketDraft::default()
    };
    let key = "DEMO-9".parse().expect("key");
    let card = crate::Ticket::new_proposed(
        crate::TicketId::new(),
        view.chain.id,
        key,
        draft,
        mark,
        Timestamp(0),
    );
    view.propose(card).expect("propose");
    let text = export(&view, &HashMap::new());
    assert!(!text.contains("token refresh"), "{text}");
    let again = parse(&text).expect("reparses");
    assert_eq!(again.tickets.len(), 4);
}

#[test]
fn design_first_is_read_instantiated_and_exported_only_when_set() {
    let mut plan = parse(DEMO).expect("demo parses");
    assert!(plan.tickets.iter().all(|t| !t.design_first));
    plan.tickets[0].design_first = true;
    let (view, _) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    assert_eq!(
        view.tickets.iter().filter(|t| t.design_first).count(),
        1,
        "only the first card asked for it"
    );
    let text = export(&view, &HashMap::new());
    assert_eq!(text.matches("design_first = true").count(), 1, "{text}");
    let again = parse(&text).expect("reparses");
    assert!(again.tickets[0].design_first);
    assert!(again.tickets[1..].iter().all(|t| !t.design_first));
    let typed = parse("[chain]\nname = \"c\"\n\n[[ticket]]\nid = \"a\"\ndesign_first = true\n")
        .expect("parses");
    assert!(typed.tickets[0].design_first);
}

#[test]
fn export_escapes_strings() {
    let mut plan = parse(DEMO).expect("demo parses");
    plan.tickets[0].title = "quote \" and\nnewline".into();
    let (view, _) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    let again = parse(&export(&view, &HashMap::new())).expect("reparses");
    assert_eq!(again.tickets[0].title, "quote \" and\nnewline");
}

#[test]
fn unknown_reference_names_its_line() {
    let e = err("[chain]\nname = \"x\"\n\n[[ticket]]\nid = \"A\"\nafter = [\"Z\"]\n");
    assert_eq!(
        e,
        PlanError {
            line: 6,
            message: "unknown id \"Z\"".into()
        }
    );
}

#[test]
fn duplicate_id_names_its_line() {
    let e = err("[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\n[[release]]\nid = \"A\"\n");
    assert_eq!(e.line, 6);
    assert_eq!(e.message, "duplicate id \"A\"");
}

#[test]
fn cycle_is_rejected_with_its_path() {
    let e = err(
        "[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\nafter = [\"B\"]\n[[ticket]]\nid = \"B\"\nafter = [\"A\"]\n",
    );
    assert_eq!(e.message, "dependency cycle: A -> B -> A");
    assert_eq!(e.line, 4);
}

#[test]
fn syntax_and_schema_errors_carry_lines() {
    let e = err("[chain]\nname = \"x\"\n\n[[ticket]]\nid = \"A\"\ntitel = \"typo\"\n");
    assert_eq!(e.line, 6, "{e}");
    assert!(e.message.contains("titel"), "{e}");
    let e = err("[chain]\nname = \"x\"\nbroken =\n");
    assert_eq!(e.line, 3, "{e}");
    let e = err("[chain]\nname = \"x\"\n[[release]]\nid = \"R\"\nstart = \"sometimes\"\n");
    assert_eq!(e.line, 5, "{e}");
}

#[test]
fn after_release_must_point_at_a_release() {
    let e = err(
        "[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\n[[ticket]]\nid = \"B\"\nafter_release = [\"A\"]\n",
    );
    assert_eq!(e.line, 7);
    assert!(
        e.message.starts_with("after_release needs a release"),
        "{e}"
    );
}

#[test]
fn self_and_duplicate_edges_are_rejected() {
    assert_eq!(
        err("[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\nafter = [\"A\"]\n").message,
        "A depends on itself"
    );
    let e = err(
        "[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\n[[ticket]]\nid = \"B\"\nafter = [\"A\", \"A\"]\n",
    );
    assert_eq!(e.message, "A -> B is listed twice");
}

#[test]
fn the_phase_1_plan_is_a_valid_chain() {
    let plan =
        parse(include_str!("../../../../plans/phase-1.toml")).expect("plans/phase-1.toml parses");
    assert_eq!(plan.chain.name, "phase-1");
    assert!(!plan.chain.autostart, "planned work waits for a person");
    assert_eq!(plan.tickets.len(), 17);
    assert_eq!(plan.releases[0].version, "v0.2.0");
    let (view, _) = plan.instantiate(ProjectId::new(), Timestamp(0), keys());
    view.validate().expect("valid graph");
}

mod order;
