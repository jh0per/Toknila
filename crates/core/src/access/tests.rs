use super::*;
use crate::conversation::{About, Author};
use crate::{ChainId, TicketStatus, Timestamp};

fn epic(project: ProjectId, autostart: bool) -> Chain {
    Chain {
        id: ChainId::new(),
        project_id: project,
        name: "plan".to_owned(),
        title: None,
        goal: String::new(),
        autostart,
        review_mode: None,
        reviewer: None,
        source: None,
        position: 0,
        version: None,
    }
}

fn story(epic: &Chain, status: TicketStatus) -> Ticket {
    let mut t = Ticket::new_backlog(
        TicketId::new(),
        epic.id,
        "DEMO-1".parse().expect("key"),
        crate::TicketDraft {
            title: "One".to_owned(),
            ..crate::TicketDraft::default()
        },
        Timestamp(1),
    );
    t.status = status;
    t
}

fn said(author: Author, about: Option<About>, text: &str) -> Message {
    Message {
        id: "m".to_owned(),
        author,
        text: text.to_owned(),
        about,
        attachments: vec![],
        thread: None,
        at: Timestamp(1),
    }
}

const ACTIONS: [Action; 13] = [
    Action::ReadPlan,
    Action::NewEpic {
        autostart: None,
        owner_fields: false,
    },
    Action::EditEpic {
        owner_fields: false,
    },
    Action::AddStory {
        owner_fields: false,
    },
    Action::EditStory {
        owner_fields: false,
    },
    Action::ChangeGraph {
        owner_fields: false,
    },
    Action::ShowStory,
    Action::Thread,
    Action::Speak,
    Action::ReportTurn,
    Action::SendEvent { verdict: false },
    Action::SendEvent { verdict: true },
    Action::RunSignal,
];

/// Every caller on every action, on a story of the caller's own project and an epic that waits
/// for the owner: what each may do, and the refusal of what it may not.
#[test]
fn every_caller_on_every_action_on_its_own_story() {
    let p = ProjectId::new();
    let c = epic(p, false);
    let t = story(&c, TicketStatus::Ready);
    let target = Target {
        project: Some((p, "DEMO")),
        epic: Some(&c),
        story: Some(&t),
        ..Target::default()
    };
    let card = |role| Caller::Card { ticket: t.id, role };
    use Refusal::*;
    const OK: Option<Refusal> = None;
    const EV: Option<Refusal> = Some(EventsOnly);
    // Columns: person, planner, worker, reviewer, release.
    #[rustfmt::skip]
    let table: [(Action, [Option<Refusal>; 5]); 13] = [
        (ACTIONS[0], [OK, OK, EV, EV, EV]),
        (ACTIONS[1], [OK, OK, EV, EV, EV]),
        (ACTIONS[2], [OK, OK, EV, EV, EV]),
        (ACTIONS[3], [OK, OK, EV, EV, EV]),
        (ACTIONS[4], [OK, OK, EV, EV, EV]),
        (ACTIONS[5], [OK, OK, EV, EV, EV]),
        (ACTIONS[6], [OK, OK, EV, EV, EV]),
        (ACTIONS[7], [OK, OK, EV, EV, EV]),
        (ACTIONS[8], [Some(PlannerOnly), OK, EV, EV, EV]),
        (ACTIONS[9], [Some(PlannerOnly), OK, EV, EV, EV]),
        (ACTIONS[10], [OK, Some(PlannerEvents), OK, Some(VerdictOnly), OK]),
        (ACTIONS[11], [OK, Some(PlannerEvents), Some(NotTheReviewer), OK, Some(NotTheReviewer)]),
        (ACTIONS[12], [OK, Some(NotPlanning), OK, OK, OK]),
    ];
    let callers = [
        Caller::Person,
        Caller::Planner(p),
        card(AgentRole::Worker),
        card(AgentRole::Reviewer),
        card(AgentRole::Release),
    ];
    for (action, expected) in table {
        for (caller, want) in callers.iter().zip(expected) {
            assert_eq!(
                may(caller, &action, &target).err(),
                want,
                "{caller:?} on {action:?}"
            );
        }
    }
    assert_eq!(
        table_actions(),
        ACTIONS.len(),
        "every action is in the table"
    );
}

/// The number of actions [`ACTIONS`] lists, counted by kind, so a new action fails here until it
/// is added to the table.
fn table_actions() -> usize {
    let kind = |a: &Action| match a {
        Action::ReadPlan => 0,
        Action::NewEpic { .. } => 1,
        Action::EditEpic { .. } => 2,
        Action::AddStory { .. } => 3,
        Action::EditStory { .. } => 4,
        Action::ChangeGraph { .. } => 5,
        Action::ShowStory => 6,
        Action::Thread => 7,
        Action::Speak => 8,
        Action::ReportTurn => 9,
        Action::SendEvent { verdict: false } => 10,
        Action::SendEvent { verdict: true } => 11,
        Action::RunSignal => 12,
    };
    let mut seen: Vec<usize> = ACTIONS.iter().map(kind).collect();
    seen.dedup();
    seen.len()
}

#[test]
fn a_planner_is_refused_every_action_on_another_projects_things_and_a_person_is_not() {
    let (mine, theirs) = (ProjectId::new(), ProjectId::new());
    let c = epic(theirs, false);
    let t = story(&c, TicketStatus::Ready);
    let on_epic = Target {
        project: Some((theirs, "OTHER")),
        epic: Some(&c),
        ..Target::default()
    };
    let on_story = Target {
        story: Some(&t),
        ..on_epic
    };
    for action in &ACTIONS[..10] {
        for (target, what) in [(&on_epic, "epic plan"), (&on_story, "story DEMO-1")] {
            let e = may(&Caller::Planner(mine), action, target).expect_err("another project");
            assert_eq!(
                e,
                Refusal::OtherProject {
                    what: what.to_owned(),
                    project: "OTHER".to_owned()
                },
                "{action:?}"
            );
            assert!(e.to_string().contains("another project"), "{e}");
            if !matches!(action, Action::Speak | Action::ReportTurn) {
                assert_eq!(may(&Caller::Person, action, target), Ok(()), "{action:?}");
            }
        }
    }
}

#[test]
fn a_planners_epic_never_starts_its_stories_and_the_owners_settings_are_the_owners() {
    let p = ProjectId::new();
    let planner = Caller::Planner(p);
    let target = Target {
        project: Some((p, "DEMO")),
        ..Target::default()
    };
    let new = |autostart, owner_fields| Action::NewEpic {
        autostart,
        owner_fields,
    };
    assert_eq!(may(&planner, &new(None, false), &target), Ok(()));
    assert_eq!(may(&planner, &new(Some(false), false), &target), Ok(()));
    for refused in [new(Some(true), false), new(None, true)] {
        let e = may(&planner, &refused, &target).expect_err("the owner's");
        assert!(matches!(e, Refusal::OwnerOnly(_)), "{e}");
        assert_eq!(may(&Caller::Person, &refused, &target), Ok(()));
    }
    assert!(!new_epic_autostart(&planner, None));
    assert!(new_epic_autostart(&Caller::Person, None));
    assert!(!new_epic_autostart(&Caller::Person, Some(false)));
    for owner in [
        Action::EditEpic { owner_fields: true },
        Action::AddStory { owner_fields: true },
        Action::EditStory { owner_fields: true },
        Action::ChangeGraph { owner_fields: true },
    ] {
        let e = may(&planner, &owner, &target).expect_err("the owner's");
        assert!(e.to_string().starts_with("only the owner sets "), "{e}");
        assert_eq!(may(&Caller::Person, &owner, &target), Ok(()));
    }
}

#[test]
fn the_assistant_names_a_known_model_and_proposes_edits_and_the_person_types_and_edits() {
    let planner = Caller::Planner(ProjectId::new());
    assert_eq!(story_model(&planner, "opus"), Ok(Some("opus".to_owned())));
    let e = story_model(&planner, "gpt").expect_err("not a model");
    assert!(e.starts_with("unknown model \"gpt\""), "{e}");
    assert_eq!(
        story_model(&Caller::Person, "gpt"),
        Ok(Some("gpt".to_owned()))
    );
    assert_eq!(story_model(&Caller::Person, " "), Ok(None));
    assert!(proposes_edits(&planner));
    assert!(!proposes_edits(&Caller::Person));
}

#[test]
fn a_planner_changes_no_graph_of_an_autostart_epic_and_no_cut_epic_and_no_started_story() {
    let p = ProjectId::new();
    let planner = Caller::Planner(p);
    let auto = epic(p, true);
    let on_auto = Target {
        project: Some((p, "DEMO")),
        epic: Some(&auto),
        ..Target::default()
    };
    let change = Action::ChangeGraph {
        owner_fields: false,
    };
    for graph in [
        change,
        Action::AddStory {
            owner_fields: false,
        },
    ] {
        let e = may(&planner, &graph, &on_auto).expect_err("autostart");
        assert!(matches!(e, Refusal::AutostartChain { .. }), "{e}");
        assert!(e.to_string().contains("only the owner starts stories"));
    }
    // Words and threads stay open in it; the person changes its graph as before.
    for open in [
        Action::EditStory {
            owner_fields: false,
        },
        Action::Thread,
    ] {
        assert_eq!(may(&planner, &open, &on_auto), Ok(()), "{open:?}");
    }
    assert_eq!(may(&Caller::Person, &change, &on_auto), Ok(()));

    let manual = epic(p, false);
    let cut = Target {
        epic: Some(&manual),
        cut: Some("0.4.0"),
        ..on_auto
    };
    let edit = Action::EditEpic {
        owner_fields: false,
    };
    let e = may(&planner, &edit, &cut).expect_err("cut");
    assert_eq!(
        e.to_string(),
        "release 0.4.0 of epic plan is cut or released: the epic can no longer change"
    );
    assert!(e.is_conflict());
    assert_eq!(may(&Caller::Person, &edit, &cut), Ok(()));

    let running = story(&manual, TicketStatus::Running);
    let started = Target {
        epic: Some(&manual),
        story: Some(&running),
        cut: None,
        ..on_auto
    };
    let edit = Action::EditStory {
        owner_fields: false,
    };
    let e = may(&planner, &edit, &started).expect_err("started");
    assert!(matches!(e, Refusal::CardStarted { .. }), "{e}");
    assert_eq!(may(&Caller::Person, &edit, &started), Ok(()));
}

#[test]
fn on_the_roadmap_a_planner_adds_and_edits_stories_only_of_the_epic_made_or_named() {
    let p = ProjectId::new();
    let planner = Caller::Planner(p);
    let c = epic(p, false);
    let t = story(&c, TicketStatus::Ready);
    let roadmap = [said(Author::Person, Some(About::Roadmap), "add hiding")];
    let named = [said(Author::Person, Some(About::Roadmap), "put it in plan")];
    let on = |turn| Target {
        project: Some((p, "DEMO")),
        epic: Some(&c),
        story: Some(&t),
        turn,
        ..Target::default()
    };
    for action in [
        Action::AddStory {
            owner_fields: false,
        },
        Action::EditStory {
            owner_fields: false,
        },
    ] {
        assert!(needs_turn(&planner, &action));
        assert!(!needs_turn(&Caller::Person, &action));
        let e = may(&planner, &action, &on(&roadmap[..])).expect_err("roadmap");
        assert!(matches!(e, Refusal::RoadmapStory { .. }), "{e}");
        assert_eq!(may(&planner, &action, &on(&named[..])), Ok(()));
        assert_eq!(may(&planner, &action, &on(&[][..])), Ok(()));
        assert_eq!(may(&Caller::Person, &action, &on(&roadmap[..])), Ok(()));
    }
    assert!(!needs_turn(
        &planner,
        &Action::ChangeGraph {
            owner_fields: false
        }
    ));
}

#[test]
fn a_storys_agent_acts_on_its_own_story_only() {
    let c = epic(ProjectId::new(), false);
    let (mine, other) = (
        story(&c, TicketStatus::Running),
        story(&c, TicketStatus::Running),
    );
    let worker = Caller::Card {
        ticket: mine.id,
        role: AgentRole::Worker,
    };
    let on = |t| Target {
        story: Some(t),
        ..Target::default()
    };
    for action in [Action::RunSignal, Action::SendEvent { verdict: false }] {
        assert_eq!(may(&worker, &action, &on(&mine)), Ok(()));
        let e = may(&worker, &action, &on(&other)).expect_err("not its story");
        assert_eq!(
            e.to_string(),
            "this agent works on another ticket, not DEMO-1"
        );
        assert_eq!(may(&Caller::Person, &action, &on(&other)), Ok(()));
    }
    assert_eq!(Caller::Person.actor(), Actor::Human);
    assert_eq!(worker.actor(), Actor::Agent);
    assert_eq!(Caller::Planner(c.project_id).actor(), Actor::Agent);
}
