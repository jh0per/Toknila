//! The planner through the use cases: what it may change, that it is kept to its own project, and
//! that nothing it does starts a card.

use toknila_core::graph::Place;
use toknila_core::{
    Actor, AgentProfile, AgentRole, ProjectId, ThreadKind, ThreadState, TicketStatus,
    planner as rules,
};

use super::Mem;
use super::runs::with_agents;
use crate::planner::{
    self, CardEdit, NewCard, NewPlanChain, SubtaskEdit, add_card, add_dependency, edit_card,
    move_card, new_chain, open_thread, remove_dependency,
};
use crate::recovery::recover;
use crate::{ErrorKind, Runner, threads};

/// DEMO-1 (A) -> DEMO-2 (B) in the chain `demo`, which does not autostart; commands run as the
/// agent the planner is.
fn manual_demo() -> Mem {
    let mut mem = Mem::demo();
    mem.view.chain.autostart = false;
    mem.actor = Actor::Agent;
    mem
}

fn project(mem: &Mem) -> ProjectId {
    mem.project.id
}

fn card(title: &str) -> NewCard {
    NewCard {
        title: title.to_owned(),
        prompt: "do it".to_owned(),
        acceptance: vec!["it works".to_owned()],
        ..NewCard::default()
    }
}

#[test]
fn a_new_card_lands_at_the_end_of_the_chain_with_its_checklist() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let id = add_card(
        &mut mem,
        p,
        "demo",
        NewCard {
            subtasks: vec!["one".to_owned(), "two".to_owned()],
            ..card("C")
        },
    )
    .expect("add");
    let c = mem.ticket_by_id(id);
    assert_eq!((c.title.as_str(), c.status), ("C", TicketStatus::Backlog));
    let view = planner::plan(&mem, p, mem.now).expect("plan");
    let cards = &view.chains[0].cards;
    let keys: Vec<String> = cards.iter().map(|c| c.ticket.key.to_string()).collect();
    assert_eq!(keys, ["DEMO-1", "DEMO-2", "DEMO-3"]);
    assert_eq!(
        cards[2].after,
        ["DEMO-2"],
        "the end of the chain waits for the last card"
    );
    assert_eq!(cards[2].subtasks.len(), 2);
    assert_eq!(cards[1].after, ["DEMO-1"]);
}

#[test]
fn a_new_chain_never_starts_its_cards_by_itself() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let id = new_chain(
        &mut mem,
        p,
        NewPlanChain {
            name: "plan".to_owned(),
            goal: Some("g".to_owned()),
        },
    )
    .expect("chain");
    assert_eq!(mem.view.chain.id, id);
    assert!(!mem.view.chain.autostart);
    assert_eq!(mem.view.chain.project_id, p);
}

#[test]
fn nothing_the_planner_does_starts_a_card_and_the_probe_can_tell() {
    // Control: a person's card in an autostart chain with a process profile does start.
    let mut control = with_agents();
    control.actor = Actor::Human;
    recover(&mut control).expect("settle");
    let started = control.ticket_by_title("A");
    assert_ne!(started.status, TicketStatus::Backlog, "the control starts");

    // The planner edits a manual chain of the same shape: no card moves, no process is queued.
    let mut mem = with_agents();
    mem.view.chain.autostart = false;
    mem.actor = Actor::Agent;
    let p = project(&mem);
    add_card(&mut mem, p, "demo", card("C")).expect("add");
    remove_dependency(&mut mem, p, "demo", "DEMO-1", "DEMO-2").expect("remove");
    add_dependency(&mut mem, p, "demo", "DEMO-2", "DEMO-1").expect("add dep");
    move_card(&mut mem, p, "demo", "DEMO-3", "DEMO-1", Place::Before).expect("move");
    edit_card(&mut mem, p, "DEMO-1", CardEdit::default()).expect("edit");
    open_thread(&mut mem, p, "DEMO-1", ThreadKind::Design, "List").expect("thread");
    assert!(
        mem.deferred.is_empty(),
        "no process was queued: {:?}",
        mem.deferred
    );
    for t in &mem.view.tickets {
        assert!(
            matches!(t.status, TicketStatus::Backlog | TicketStatus::Ready),
            "{} is {:?}",
            t.key,
            t.status
        );
        assert!(!t.queued);
    }
}

#[test]
fn the_order_of_cards_can_be_changed_with_dependencies_and_moves() {
    let mut mem = manual_demo();
    let p = project(&mem);
    add_card(&mut mem, p, "demo", card("C")).expect("add");
    move_card(&mut mem, p, "demo", "DEMO-3", "DEMO-1", Place::Before).expect("move");
    let view = planner::plan(&mem, p, mem.now).expect("plan");
    let keys: Vec<String> = view.chains[0]
        .cards
        .iter()
        .map(|c| c.ticket.key.to_string())
        .collect();
    assert_eq!(keys, ["DEMO-3", "DEMO-1", "DEMO-2"]);
    remove_dependency(&mut mem, p, "demo", "DEMO-1", "DEMO-2").expect("remove");
    let view = planner::plan(&mem, p, mem.now).expect("plan");
    let b = view.chains[0]
        .cards
        .iter()
        .find(|c| c.ticket.title == "B")
        .expect("B");
    assert!(!b.after.contains(&"DEMO-1".to_owned()));
    let gone = remove_dependency(&mut mem, p, "demo", "DEMO-1", "DEMO-2").expect_err("no edge");
    assert_eq!(gone.kind, ErrorKind::NotFound);
}

#[test]
fn a_chain_that_autostarts_refuses_graph_changes_but_not_text_or_threads() {
    let mut mem = Mem::demo();
    mem.actor = Actor::Agent;
    let p = project(&mem);
    let refusals = [
        add_card(&mut mem, p, "demo", card("C")).map(|_| ()),
        add_dependency(&mut mem, p, "demo", "DEMO-2", "DEMO-1").map(|_| ()),
        remove_dependency(&mut mem, p, "demo", "DEMO-1", "DEMO-2").map(|_| ()),
        move_card(&mut mem, p, "demo", "DEMO-2", "DEMO-1", Place::Before).map(|_| ()),
    ];
    for r in refusals {
        let e = r.expect_err("refused");
        assert_eq!(e.kind, ErrorKind::Forbidden, "{e}");
        assert!(e.message.contains("only the owner starts cards"), "{e}");
    }
    assert_eq!(mem.view.tickets.len(), 2, "nothing was added");
    let edit = CardEdit {
        title: Some("A2".to_owned()),
        ..CardEdit::default()
    };
    edit_card(&mut mem, p, "DEMO-1", edit).expect("text edits are open");
    open_thread(&mut mem, p, "DEMO-1", ThreadKind::Other, "Q").expect("threads are open");
}

#[test]
fn only_a_card_that_has_not_started_is_edited_and_its_checklist_with_it() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let edit = CardEdit {
        title: Some("A2".to_owned()),
        prompt: Some("new prompt".to_owned()),
        acceptance: Some(vec!["a".to_owned(), "b".to_owned()]),
        add_subtasks: vec!["first".to_owned()],
        edit_subtasks: vec![],
    };
    let id = edit_card(&mut mem, p, "DEMO-1", edit).expect("edit");
    let t = mem.ticket_by_id(id);
    assert_eq!(t.title, "A2");
    assert_eq!(t.prompt, "new prompt");
    assert_eq!(t.acceptance, ["a", "b"]);
    let rename = CardEdit {
        edit_subtasks: vec![SubtaskEdit {
            position: 1,
            text: "renamed".to_owned(),
        }],
        ..CardEdit::default()
    };
    edit_card(&mut mem, p, "DEMO-1", rename).expect("rename");
    assert_eq!(mem.subtasks[0].text, "renamed");

    mem.view.tickets[0].status = TicketStatus::Running;
    let e = edit_card(&mut mem, p, "DEMO-1", CardEdit::default()).expect_err("started");
    assert_eq!(e.kind, ErrorKind::Forbidden);
    assert!(e.message.contains("have not started"), "{e}");
}

#[test]
fn a_planner_stays_in_its_own_project() {
    let mut mem = manual_demo();
    let other = ProjectId::new();
    let again = |r: Result<_, crate::AppError>| -> crate::AppError {
        match r {
            Ok(_) => panic!("the other project's planner was let in"),
            Err(e) => e,
        }
    };
    let errors = [
        again(edit_card(&mut mem, other, "DEMO-1", CardEdit::default()).map(|_| ())),
        again(add_card(&mut mem, other, "demo", card("C")).map(|_| ())),
        again(add_dependency(&mut mem, other, "demo", "DEMO-1", "DEMO-2").map(|_| ())),
        again(remove_dependency(&mut mem, other, "demo", "DEMO-1", "DEMO-2").map(|_| ())),
        again(move_card(&mut mem, other, "demo", "DEMO-2", "DEMO-1", Place::Before).map(|_| ())),
        again(open_thread(&mut mem, other, "DEMO-1", ThreadKind::Design, "x").map(|_| ())),
        again(planner::thread(&mem, other, "DEMO-1", "1").map(|_| ())),
    ];
    for e in errors {
        assert_eq!(e.kind, ErrorKind::Forbidden, "{e}");
        assert!(e.message.contains("another project"), "{e}");
    }
    // The control: the project's own planner gets through the same calls.
    let own = project(&mem);
    edit_card(&mut mem, own, "DEMO-1", CardEdit::default()).expect("own project");
    // By id, too: the id of a card of another project is not a way in.
    let id = mem.ticket_by_title("A").id.to_string();
    let e = again(edit_card(&mut mem, other, &id, CardEdit::default()).map(|_| ()));
    assert_eq!(e.kind, ErrorKind::Forbidden);
    let missing = again(edit_card(&mut mem, own, "DEMO-99", CardEdit::default()).map(|_| ()));
    assert_eq!(missing.kind, ErrorKind::NotFound);
}

#[test]
fn the_planner_talks_in_threads_and_only_the_owner_approves() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let (a, th) =
        open_thread(&mut mem, p, "DEMO-1", ThreadKind::Flow, "Order of work").expect("open");
    assert_eq!(
        (th.number, th.state, th.opened_by),
        (1, ThreadState::Open, Actor::Agent)
    );
    let (a2, th) = planner::thread(&mem, p, "DEMO-1", "1").expect("find");
    assert_eq!(a2.id, a.id);
    threads::post_message(&mut mem, &a, &th, "Which first?").expect("message");
    assert_eq!(mem.messages.len(), 1);
    assert_eq!(mem.messages[0].author, Actor::Agent);
    let e = threads::approve(&mut mem, &a, &th).expect_err("the planner does not approve");
    assert!(!e.message.is_empty());
    assert_eq!(mem.threads[0].state, ThreadState::Open);
    let view = planner::plan(&mem, p, mem.now).expect("plan");
    let seen = &view.chains[0].cards[0].threads;
    assert_eq!((seen.len(), seen[0].messages.len()), (1, 1));
}

#[test]
fn starting_the_planner_is_a_persons_action_in_the_repository_with_a_planner_profile() {
    let mut mem = with_agents();
    let planner_profile = AgentProfile::claude_code_planner();
    let key = rules::run_key(mem.project.id);

    let e = planner::start(
        &mem.runner,
        Actor::Agent,
        &mem.project,
        &planner_profile,
        None,
    )
    .expect_err("not an agent's: person-only is a conflict, like every person-only command");
    assert_eq!(e.kind, ErrorKind::Conflict);

    let started = planner::start(
        &mem.runner,
        Actor::Human,
        &mem.project,
        &planner_profile,
        None,
    )
    .expect("start");
    assert!(!started.reused);
    let req = mem.runner.started.borrow()[0].clone();
    assert_eq!((req.ticket, req.role), (key, AgentRole::Planner));
    assert_eq!(req.dir, "/repo");
    assert_eq!(req.key.to_string(), "DEMO-0");
    let facts = req.brief.project.expect("the project's facts");
    assert_eq!(
        (facts.key.as_str(), facts.work_branch.as_str()),
        ("DEMO", "develop")
    );
    assert!(planner::is_live(&mem.runner, &mem.project));
    assert!(
        planner::start(
            &mem.runner,
            Actor::Human,
            &mem.project,
            &planner_profile,
            None
        )
        .expect("again")
        .reused
    );

    planner::send_input(&mem.runner, Actor::Human, &mem.project, b"hi\r")
        .expect_err("only workers in the fake");
    let e = planner::send_input(&mem.runner, Actor::Agent, &mem.project, b"x").expect_err("person");
    assert_eq!(e.kind, ErrorKind::Conflict);

    let e = planner::stop(&mem.runner, Actor::Agent, &mem.project).expect_err("person");
    assert_eq!(e.kind, ErrorKind::Conflict);
    planner::stop(&mem.runner, Actor::Human, &mem.project).expect("stop");
    assert!(!planner::is_live(&mem.runner, &mem.project));
    assert_eq!(mem.runner.live_runners(), vec![]);

    // A worker profile is not a planner; a project without a repository has nothing to read.
    let worker = AgentProfile::claude_code();
    let e =
        planner::start(&mem.runner, Actor::Human, &mem.project, &worker, None).expect_err("worker");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    mem.project.repo_path = None;
    let e = planner::start(
        &mem.runner,
        Actor::Human,
        &mem.project,
        &planner_profile,
        None,
    )
    .expect_err("no repository");
    assert_eq!(e.kind, ErrorKind::Conflict);
}
