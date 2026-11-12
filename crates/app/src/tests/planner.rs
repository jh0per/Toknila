//! The planner through the use cases: what it may change, that it is kept to its own project, and
//! that nothing it does starts a card.

use toknila_core::graph::Place;
use toknila_core::{
    Actor, AgentProfile, AgentRole, ProjectId, ThreadKind, ThreadState, TicketStatus,
    planner as rules, suggestion::SuggestionAction,
};

use super::Mem;
use super::runs::with_agents;
use crate::planner::{
    self, CardEdit, EditPlanChain, NewCard, NewPlanChain, SubtaskEdit, add_card, add_dependency,
    edit_card, edit_chain, move_card, new_chain, open_thread, remove_dependency,
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
            version: Some("v0.4.0".to_owned()),
        },
    )
    .expect("chain");
    assert_eq!(mem.view.chain.id, id);
    assert!(!mem.view.chain.autostart);
    assert_eq!(mem.view.chain.project_id, p);
    assert_eq!(mem.view.chain.version.as_deref(), Some("0.4.0"));
}

#[test]
fn the_planner_edits_name_goal_and_version_of_an_epic_whose_release_is_not_cut() {
    let mut mem = manual_demo();
    let p = project(&mem);
    new_chain(
        &mut mem,
        p,
        NewPlanChain {
            name: "plan".to_owned(),
            version: Some("0.4.0".to_owned()),
            ..NewPlanChain::default()
        },
    )
    .expect("chain");
    edit_chain(
        &mut mem,
        p,
        "plan",
        EditPlanChain {
            name: Some("phase-4".to_owned()),
            goal: Some("ship it".to_owned()),
            version: Some("0.5.0".to_owned()),
        },
    )
    .expect("edit");
    let c = &mem.view.chain;
    assert_eq!(
        (c.name.as_str(), c.goal.as_str(), c.version.as_deref()),
        ("phase-4", "ship it", Some("0.5.0"))
    );
    assert_eq!(mem.view.finish_release().expect("release").version, "0.5.0");

    let end = mem.view.finish_release().expect("release").id;
    mem.view.cut_release(end, mem.now).expect("cut");
    let refused = edit_chain(
        &mut mem,
        p,
        "phase-4",
        EditPlanChain {
            name: Some("other".to_owned()),
            ..EditPlanChain::default()
        },
    )
    .expect_err("cut");
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert_eq!(mem.view.chain.name, "phase-4");
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
        design_first: Some(true),
        designs: None,
        model: None,
        add_subtasks: vec!["first".to_owned()],
        edit_subtasks: vec![],
    };
    let id = edit_card(&mut mem, p, "DEMO-1", edit).expect("edit");
    let t = mem.ticket_by_id(id);
    assert!(t.design_first);
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
fn showing_a_card_emits_one_window_event_and_changes_nothing_else() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let before = mem.events.len();
    let t = planner::show_card(&mut mem, p, "DEMO-2").expect("show");
    assert_eq!(t.key.to_string(), "DEMO-2");
    assert_eq!(mem.events.len(), before + 1);
    match mem.events.last() {
        Some(toknila_core::events::DomainEvent::WindowShowCard {
            project,
            project_key,
            chain,
            ticket,
        }) => assert_eq!(
            (
                *project,
                project_key.to_string(),
                chain.as_str(),
                ticket.to_string()
            ),
            (p, "DEMO".to_owned(), "demo", "DEMO-2".to_owned())
        ),
        other => panic!("{other:?}"),
    }
    // Another project's planner is refused and nothing is emitted; a missing card is not found.
    let e = planner::show_card(&mut mem, ProjectId::new(), "DEMO-1").expect_err("other");
    assert_eq!(e.kind, ErrorKind::Forbidden);
    let e = planner::show_card(&mut mem, p, "DEMO-99").expect_err("missing");
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert_eq!(mem.events.len(), before + 1);
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

#[test]
fn the_planner_is_started_on_the_planner_model_of_its_project() {
    let mut mem = Mem::demo();
    mem.project.repo_path = Some("/work/demo".to_owned());
    mem.project.worker_model = Some("sonnet".to_owned());
    let profile = AgentProfile::claude_code_planner();
    let req = planner::request(&mem.project, &profile).expect("request");
    assert_eq!(
        req.brief.model, None,
        "the worker's model is not the planner's"
    );
    mem.project.planner_model = Some("opus".to_owned());
    let req = planner::request(&mem.project, &profile).expect("request");
    assert_eq!(req.brief.model.as_deref(), Some("opus"));
}

#[test]
fn a_card_takes_a_model_by_alias_or_full_name_and_a_wrong_one_is_refused_with_the_aliases() {
    let mut mem = manual_demo();
    let p = project(&mem);
    let id = add_card(
        &mut mem,
        p,
        "demo",
        NewCard {
            model: Some("fable".to_owned()),
            ..card("C")
        },
    )
    .expect("an alias");
    assert_eq!(mem.ticket_by_id(id).model.as_deref(), Some("fable"));
    let e = add_card(
        &mut mem,
        p,
        "demo",
        NewCard {
            model: Some("gpt".to_owned()),
            ..card("D")
        },
    )
    .expect_err("gpt is no model of ours");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    assert!(
        e.message.contains("sonnet, opus, haiku, fable"),
        "{}",
        e.message
    );
    let edit = |model: &str| CardEdit {
        model: Some(model.to_owned()),
        ..CardEdit::default()
    };
    let one = edit_card(&mut mem, p, "DEMO-1", edit("opus")).expect("edit");
    assert_eq!(mem.ticket_by_id(one).model.as_deref(), Some("opus"));
    assert!(edit_card(&mut mem, p, "DEMO-1", edit("gpt")).is_err());
    assert_eq!(mem.ticket_by_id(one).model.as_deref(), Some("opus"));
    edit_card(&mut mem, p, "DEMO-1", edit("")).expect("clear");
    assert_eq!(mem.ticket_by_id(one).model, None);
}

fn ready(mem: &mut Mem, key: &str, status: TicketStatus) {
    let t = mem
        .view
        .tickets
        .iter_mut()
        .find(|t| t.key.to_string() == key)
        .expect("card");
    t.status = status;
}

fn say(text: &str, card: Option<&str>, action: Option<SuggestionAction>) -> planner::NewSuggestion {
    planner::NewSuggestion {
        text: text.to_owned(),
        card: card.map(str::to_owned),
        action,
    }
}

#[test]
fn a_suggestion_is_kept_with_its_card_and_replaced_by_the_next_one() {
    let mut mem = manual_demo();
    let p = project(&mem);
    ready(&mut mem, "DEMO-1", TicketStatus::Ready);
    let first = planner::suggest(
        &mut mem,
        p,
        say(
            "Start DEMO-1",
            Some("DEMO-1"),
            Some(SuggestionAction::Start),
        ),
    )
    .expect("suggest");
    assert_eq!(
        first.card.as_ref().map(ToString::to_string).as_deref(),
        Some("DEMO-1")
    );
    assert_eq!(first.chain, Some(mem.view.chain.id));
    assert_eq!(mem.suggestion.as_ref(), Some(&first));
    let second =
        planner::suggest(&mut mem, p, say("Nothing waits for you", None, None)).expect("replace");
    assert_eq!((&second.card, second.action), (&None, None));
    assert_eq!(mem.suggestion.as_ref(), Some(&second));
    assert_eq!(
        mem.kinds()
            .iter()
            .filter(|k| *k == "window.suggest")
            .count(),
        2
    );
    // Nothing in the plan moved and nothing started.
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Ready);
}

#[test]
fn a_suggestion_the_card_cannot_take_now_or_about_another_project_is_refused() {
    let mut mem = manual_demo();
    let p = project(&mem);
    // DEMO-1 is in Backlog: there is nothing to accept, answer or (blocked by a dependency) start.
    for action in [SuggestionAction::Accept, SuggestionAction::Answer] {
        let e = planner::suggest(&mut mem, p, say("x", Some("DEMO-1"), Some(action)))
            .expect_err("not now");
        assert_eq!(e.kind, ErrorKind::BadRequest, "{action:?}");
    }
    let e = planner::suggest(&mut mem, p, say("x", Some("DEMO-1"), None)).expect_err("no action");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    let e = planner::suggest(&mut mem, p, say("", None, None)).expect_err("no text");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    let e = planner::suggest(
        &mut mem,
        ProjectId::new(),
        say("x", Some("DEMO-1"), Some(SuggestionAction::Start)),
    )
    .expect_err("other project");
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert_eq!(mem.suggestion, None);
}

#[test]
fn the_planner_sets_changes_and_clears_a_cards_designs() {
    use toknila_core::design::{DesignRef, Region};
    let mut mem = manual_demo();
    let p = project(&mem);
    let (login, home) = (toknila_core::DesignId::new(), toknila_core::DesignId::new());
    let region = Region {
        x: 5,
        y: 6,
        w: 7,
        h: 8,
    };
    let with = vec![
        DesignRef {
            design: login,
            region: Some(region),
        },
        DesignRef {
            design: home,
            region: None,
        },
    ];
    let id =
        planner::add_card_with_designs(&mut mem, p, "demo", card("C"), with.clone()).expect("add");
    assert_eq!(mem.ticket_by_id(id).designs, with);
    // A change replaces the list; leaving it out keeps it.
    let key = mem.ticket_by_id(id).key.to_string();
    planner::edit_card(
        &mut mem,
        p,
        &key,
        CardEdit {
            title: Some("C2".into()),
            ..CardEdit::default()
        },
    )
    .expect("edit");
    assert_eq!(mem.ticket_by_id(id).designs, with);
    let one = vec![DesignRef {
        design: home,
        region: None,
    }];
    planner::edit_card_with_designs(&mut mem, p, &key, CardEdit::default(), Some(one.clone()))
        .expect("replace");
    assert_eq!(mem.ticket_by_id(id).designs, one);
    planner::edit_card_with_designs(&mut mem, p, &key, CardEdit::default(), Some(vec![]))
        .expect("clear");
    assert!(mem.ticket_by_id(id).designs.is_empty());
    // A region outside the image is a bad request and changes nothing.
    let bad = vec![DesignRef {
        design: home,
        region: Some(Region {
            x: 90,
            y: 0,
            w: 20,
            h: 5,
        }),
    }];
    let e = planner::edit_card_with_designs(&mut mem, p, &key, CardEdit::default(), Some(bad))
        .expect_err("outside");
    assert_eq!(e.kind, crate::ErrorKind::BadRequest);
}
