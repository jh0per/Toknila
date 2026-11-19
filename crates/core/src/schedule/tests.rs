use super::*;
use crate::{Chain, ChainId, Edge, EdgeKind, NodeRef, ProjectId, Timestamp};

struct World {
    project: ProjectId,
    chains: Vec<ChainView>,
    next: u32,
}

impl World {
    fn new() -> Self {
        Self {
            project: ProjectId::new(),
            chains: Vec::new(),
            next: 1,
        }
    }

    fn chain(&mut self, autostart: bool) -> usize {
        let chain = Chain {
            id: ChainId::new(),
            project_id: self.project,
            name: format!("c{}", self.chains.len()),
            title: None,
            goal: String::new(),
            autostart,
            review_mode: None,
            reviewer: None,
            source: None,
            position: 0,
            version: None,
        };
        self.chains.push(ChainView {
            chain,
            tickets: vec![],
            releases: vec![],
            edges: vec![],
            release_tickets: vec![],
        });
        self.chains.len() - 1
    }

    fn card(&mut self, chain: usize, status: TicketStatus, queued: bool) -> TicketId {
        let view = &mut self.chains[chain];
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            view.chain.id,
            format!("DEMO-{}", self.next).parse().expect("key"),
            crate::TicketDraft::default(),
            Timestamp(0),
        );
        self.next += 1;
        t.status = status;
        t.queued = queued;
        if status == TicketStatus::Waiting {
            t.status_reason = Some(crate::StatusReason::Waiting(crate::WaitingReason::Silent));
        }
        let id = t.id;
        view.tickets.push(t);
        id
    }

    fn after(&mut self, chain: usize, from: TicketId, to: TicketId) {
        let view = &mut self.chains[chain];
        view.edges.push(Edge {
            chain_id: view.chain.id,
            from: NodeRef::Ticket(from),
            to: NodeRef::Ticket(to),
            kind: EdgeKind::Normal,
        });
    }

    fn plan(&self, limit: u32) -> Plan {
        plan(&self.chains, limit, &[])
    }
}

#[test]
fn the_limit_is_respected_and_the_rest_waits_in_order() {
    let mut w = World::new();
    let c = w.chain(false);
    let a = w.card(c, TicketStatus::Ready, true);
    let b = w.card(c, TicketStatus::Ready, true);
    let d = w.card(c, TicketStatus::Ready, true);
    let p = w.plan(2);
    assert_eq!((p.used, p.start, p.queue), (0, vec![a, b], vec![d]));
    assert_eq!(w.plan(2).position(d), Some(1));
    assert_eq!(w.plan(2).position(a), None);
}

#[test]
fn running_and_waiting_cards_hold_slots_and_nothing_else_does() {
    let mut w = World::new();
    let c = w.chain(false);
    for status in [
        TicketStatus::Review,
        TicketStatus::Blocked,
        TicketStatus::Merged,
        TicketStatus::Cancelled,
        TicketStatus::Backlog,
    ] {
        w.card(c, status, false);
    }
    assert_eq!(w.plan(2).used, 0);
    w.card(c, TicketStatus::Running, false);
    w.card(c, TicketStatus::Waiting, false);
    let next = w.card(c, TicketStatus::Ready, true);
    let p = w.plan(2);
    assert_eq!((p.used, p.start, p.queue), (2, vec![], vec![next]));
}

#[test]
fn a_freed_slot_starts_the_next_card() {
    let mut w = World::new();
    let c = w.chain(false);
    let a = w.card(c, TicketStatus::Running, false);
    let b = w.card(c, TicketStatus::Running, false);
    let q = w.card(c, TicketStatus::Ready, true);
    assert_eq!(w.plan(2).queue, vec![q]);
    w.chains[c]
        .tickets
        .iter_mut()
        .find(|t| t.id == a)
        .expect("a")
        .status = TicketStatus::Review;
    let p = w.plan(2);
    assert_eq!((p.used, p.start), (1, vec![q]));
    assert!(p.queue.is_empty());
    let _ = b;
}

#[test]
fn the_queue_follows_chain_order_then_the_graph() {
    let mut w = World::new();
    let first = w.chain(false);
    let second = w.chain(false);
    let a1 = w.card(first, TicketStatus::Ready, true);
    let a2 = w.card(first, TicketStatus::Ready, true);
    let b1 = w.card(second, TicketStatus::Ready, true);
    // A card created later than the other chain's, in an earlier chain, still goes first, and
    // a card that waits for an unfinished one is not in line yet.
    let a3 = w.card(first, TicketStatus::Ready, true);
    w.after(first, a1, a3);
    let p = w.plan(1);
    assert_eq!(p.start, vec![a1]);
    assert_eq!(p.queue, vec![a2, b1]);
    assert_eq!(p.position(b1), Some(2));
    let _ = a3;
}

#[test]
fn dependencies_are_honoured() {
    let mut w = World::new();
    let c = w.chain(true);
    let done = w.card(c, TicketStatus::Merged, false);
    let ready = w.card(c, TicketStatus::Ready, false);
    let blocked_by_open = w.card(c, TicketStatus::Ready, false);
    let open = w.card(c, TicketStatus::Review, false);
    w.after(c, done, ready);
    w.after(c, open, blocked_by_open);
    assert_eq!(w.plan(5).start, vec![ready]);
}

#[test]
fn an_unrequested_card_is_never_started_but_an_autostart_chain_asks_for_all() {
    let mut w = World::new();
    let manual = w.chain(false);
    let auto = w.chain(true);
    let idle = w.card(manual, TicketStatus::Ready, false);
    let wanted = w.card(auto, TicketStatus::Ready, false);
    let p = w.plan(5);
    assert_eq!(p.start, vec![wanted]);
    assert!(!p.start.contains(&idle) && !p.queue.contains(&idle));
}

#[test]
fn lowering_the_limit_below_use_starts_nothing_and_raising_it_starts_the_queue() {
    let mut w = World::new();
    let c = w.chain(false);
    w.card(c, TicketStatus::Running, false);
    w.card(c, TicketStatus::Running, false);
    let q1 = w.card(c, TicketStatus::Ready, true);
    let q2 = w.card(c, TicketStatus::Ready, true);
    let down = w.plan(1);
    assert_eq!((down.used, down.start.len(), down.queue.len()), (2, 0, 2));
    let up = w.plan(3);
    assert_eq!((up.start, up.queue), (vec![q1], vec![q2]));
}

#[test]
fn limits_outside_the_range_are_refused() {
    assert_eq!(check_limit(1), Ok(1));
    assert_eq!(check_limit(MAX_LIMIT), Ok(MAX_LIMIT));
    assert_eq!(check_limit(0), Err(LimitError(0)));
    assert_eq!(check_limit(MAX_LIMIT + 1), Err(LimitError(MAX_LIMIT + 1)));
}

#[test]
fn a_story_holds_one_slot_whatever_agents_of_it_are_alive() {
    let mut w = World::new();
    let c = w.chain(false);
    let reviewed = w.card(c, TicketStatus::Review, false);
    let running = w.card(c, TicketStatus::Running, false);
    let third = w.card(c, TicketStatus::Ready, true);
    // Without runners a card in review holds nothing: one slot used, the third starts.
    let none = plan(&w.chains, 2, &[]);
    assert_eq!((none.used, none.start.clone()), (1, vec![third]));
    // Its worker still ending and its reviewer alive together are the story's one slot: with
    // the running card the limit of 2 is reached and the third waits.
    let live = [
        (reviewed, AgentRole::Worker),
        (reviewed, AgentRole::Reviewer),
    ];
    let p = plan(&w.chains, 2, &live);
    assert_eq!((p.used, p.start, p.queue), (2, vec![], vec![third]));
    assert_eq!(plan(&w.chains, 3, &live).start, vec![third]);
    // A running card's live worker is the same slot, counted once; a stranger's runner is not ours.
    let same = [
        (running, AgentRole::Worker),
        (TicketId::new(), AgentRole::Worker),
    ];
    assert_eq!(plan(&w.chains, 5, &same).used, 1);
}

/// A card of chain `c` that has begun and goes on with no agent yet: sent back or unblocked
/// (`why`), answered (`None`), or lost with the app.
fn begun(w: &mut World, c: usize, why: Option<crate::GoOn>, lost: bool) -> TicketId {
    let status = if why.is_some() || lost {
        TicketStatus::Running
    } else {
        TicketStatus::Waiting
    };
    let id = w.card(c, status, false);
    let t = w.chains[c].tickets.last_mut().expect("card");
    t.agent_paused = Some(crate::PausedAgent {
        session: None,
        since: Timestamp(0),
        resume: true,
        lost,
        go_on: why,
    });
    id
}

/// A card of chain `c` in Review whose reviewer agent is due (`waits`) or working.
fn reviewing(w: &mut World, c: usize, waits: bool) -> TicketId {
    let id = w.card(c, TicketStatus::Review, false);
    let t = w.chains[c].tickets.last_mut().expect("card");
    t.review.turn = Some(crate::ReviewTurn::Agent { decides: true });
    t.review.reviewer_waits = waits;
    id
}

const SENT_BACK: Option<crate::GoOn> = Some(crate::GoOn::SentBack);

#[test]
fn a_story_sent_back_while_every_slot_is_taken_waits_and_says_so() {
    let mut w = World::new();
    let c = w.chain(false);
    let busy = w.card(c, TicketStatus::Running, false);
    let sent = begun(&mut w, c, SENT_BACK, false);
    let p = w.plan(1);
    assert_eq!(
        (p.used, p.start.clone(), p.queue.clone()),
        (1, vec![], vec![sent])
    );
    assert_eq!(
        p.text(sent),
        Some("sent back; goes on when an agent is free")
    );
    assert_eq!(p.text(busy), None);
    // The other leaves: the story sent back goes on, with the event that starts a begun story.
    w.chains[c].tickets[0].status = TicketStatus::Review;
    assert_eq!(w.plan(1).start, vec![sent]);
    let t = &w.chains[c].tickets[1];
    assert_eq!(start_event(t), TicketEvent::ResumeStart);
    assert_eq!(start_event(&w.chains[c].tickets[0]), TicketEvent::Start);
}

#[test]
fn stories_already_begun_go_before_stories_that_have_not_started_each_in_chain_order() {
    let mut w = World::new();
    let first = w.chain(false);
    let second = w.chain(false);
    let _busy = w.card(first, TicketStatus::Running, false);
    let fresh_a = w.card(first, TicketStatus::Ready, true);
    let reviewer = reviewing(&mut w, first, true);
    let fresh_b = w.card(second, TicketStatus::Ready, true);
    let sent = begun(&mut w, second, SENT_BACK, false);
    let unblocked = begun(
        &mut w,
        second,
        Some(crate::GoOn::Unblocked {
            stale: crate::StalePolicy::Allow,
        }),
        false,
    );
    let answered = begun(&mut w, second, None, false);
    let lost = begun(&mut w, second, None, true);
    let full = w.plan(1);
    assert_eq!(
        full.queue,
        vec![reviewer, sent, unblocked, answered, lost, fresh_a, fresh_b]
    );
    let words: Vec<&str> = full.texts.iter().map(String::as_str).collect();
    assert_eq!(
        words,
        [
            "the reviewer is due; it starts when an agent is free",
            "sent back; goes on when an agent is free (#2 in line)",
            "unblocked; goes on when an agent is free (#3 in line)",
            "answered; goes on when an agent is free (#4 in line)",
            "the app restarted; goes on when an agent is free (#5 in line)",
            "queued #6",
            "queued #7",
        ]
    );
    // Two free slots go to the work in progress first.
    let free = w.plan(3);
    assert_eq!(free.start, vec![reviewer, sent]);
    assert_eq!(free.position(unblocked), Some(1));
}

#[test]
fn a_reviewer_that_is_due_waits_for_a_free_slot_and_its_turn_holds_one_once_started() {
    let mut w = World::new();
    let c = w.chain(false);
    let busy = w.card(c, TicketStatus::Running, false);
    let due = reviewing(&mut w, c, true);
    let fresh = w.card(c, TicketStatus::Ready, true);
    let p = w.plan(1);
    assert_eq!((p.used, p.queue), (1, vec![due, fresh]));
    // The worker leaves (a hand-in, a block): the reviewer gets the slot, not the fresh card.
    w.chains[c].tickets[0].status = TicketStatus::Blocked;
    let p = w.plan(1);
    assert_eq!((p.start, p.queue), (vec![due], vec![fresh]));
    // Started, the reviewer's turn holds the slot by itself, simulated or not.
    w.chains[c].tickets[1].review.reviewer_waits = false;
    let p = w.plan(1);
    assert_eq!((p.used, p.start, p.queue), (1, vec![], vec![fresh]));
    assert_eq!(working_cards(&w.chains, &[]), [due]);
    let _ = busy;
}

#[test]
fn a_story_goes_on_in_the_slot_its_own_live_runner_holds_but_never_past_the_limit() {
    let mut w = World::new();
    let c = w.chain(false);
    let fresh = w.card(c, TicketStatus::Ready, true);
    let sent = begun(&mut w, c, SENT_BACK, false);
    // Its reviewer is still ending after its verdict: that is the story's slot, and the worker
    // goes on in it while the card that has not started waits.
    let ending = [(sent, AgentRole::Reviewer)];
    let p = plan(&w.chains, 1, &ending);
    assert_eq!((p.used, p.start, p.queue), (1, vec![sent], vec![fresh]));
    // A hand-in: the worker is still ending while the reviewer is due; the reviewer goes on.
    let due = reviewing(&mut w, c, true);
    let handing_in = [(due, AgentRole::Worker), (sent, AgentRole::Reviewer)];
    let p = plan(&w.chains, 2, &handing_in);
    assert_eq!((p.used, p.start), (2, vec![sent, due]));
    // Past the limit (another story holds the only slot) the story waits for it to leave.
    w.card(c, TicketStatus::Running, false);
    let p = plan(&w.chains, 1, &ending);
    assert_eq!(
        (p.used, p.start, p.queue),
        (2, vec![], vec![sent, due, fresh])
    );
}

#[test]
fn an_agent_past_the_limit_is_counted_and_delays_the_line_until_it_leaves() {
    let mut w = World::new();
    let c = w.chain(false);
    w.card(c, TicketStatus::Running, false);
    w.card(c, TicketStatus::Running, false);
    let sent = begun(&mut w, c, SENT_BACK, false);
    let p = w.plan(1);
    assert_eq!((p.used, p.start, p.queue), (2, vec![], vec![sent]));
    w.chains[c].tickets[0].status = TicketStatus::Review;
    assert_eq!(w.plan(1).queue, vec![sent], "still at the limit");
    w.chains[c].tickets[1].status = TicketStatus::Review;
    assert_eq!(w.plan(1).start, vec![sent]);
}

#[test]
fn working_cards_are_those_that_hold_a_slot_or_have_a_live_runner_in_review() {
    let mut w = World::new();
    let c = w.chain(false);
    let running = w.card(c, TicketStatus::Running, false);
    let waiting = w.card(c, TicketStatus::Waiting, false);
    let reviewed = w.card(c, TicketStatus::Review, false);
    let idle_review = w.card(c, TicketStatus::Review, false);
    let _ready = w.card(c, TicketStatus::Ready, false);
    let live = [
        (reviewed, AgentRole::Reviewer),
        (TicketId::new(), AgentRole::Worker),
    ];
    assert_eq!(
        working_cards(&w.chains, &live),
        [running, waiting, reviewed]
    );
    assert!(!working_cards(&w.chains, &[]).contains(&idle_review));
}

fn paused(w: &mut World, c: usize, resume: bool) -> TicketId {
    let id = w.card(c, TicketStatus::Waiting, false);
    let t = w.chains[c].tickets.last_mut().expect("card");
    t.agent_paused = Some(crate::PausedAgent {
        session: Some("s-1".to_owned()),
        since: Timestamp(0),
        resume,
        lost: false,
        go_on: None,
    });
    id
}

#[test]
fn a_waiting_card_with_a_paused_agent_holds_no_slot_and_the_next_card_starts() {
    let mut w = World::new();
    let c = w.chain(false);
    let _alive = w.card(c, TicketStatus::Waiting, false);
    let _gone = paused(&mut w, c, false);
    let next = w.card(c, TicketStatus::Ready, true);
    // Two slots, a Waiting card with its agent alive and a Waiting card without one.
    let p = w.plan(2);
    assert_eq!((p.used, p.start), (1, vec![next]));
    // A Waiting card with an agent and no pause still holds its slot.
    let q = plan(&w.chains, 1, &[]);
    assert_eq!((q.used, q.start, q.queue), (1, vec![], vec![next]));
}

#[test]
fn a_resumed_agent_is_ahead_of_cards_that_never_started_in_line_and_at_a_free_slot() {
    let mut w = World::new();
    let c = w.chain(false);
    let _busy = w.card(c, TicketStatus::Running, false);
    let fresh = w.card(c, TicketStatus::Ready, true);
    let answered = paused(&mut w, c, true);
    let full = w.plan(1);
    assert_eq!(full.queue, vec![answered, fresh]);
    assert_eq!(full.position(answered), Some(1));
    // One free slot goes to the agent that was already working.
    let free = w.plan(2);
    assert_eq!((free.start, free.queue), (vec![answered], vec![fresh]));
}

#[test]
fn a_paused_card_with_an_answer_starts_when_a_slot_is_free_and_queues_otherwise() {
    let mut w = World::new();
    let c = w.chain(false);
    let _busy = w.card(c, TicketStatus::Running, false);
    let answered = paused(&mut w, c, true);
    let silent = paused(&mut w, c, false);
    let full = w.plan(1);
    assert_eq!(
        (full.start.clone(), full.queue.clone()),
        (vec![], vec![answered])
    );
    assert_eq!(full.position(answered), Some(1));
    assert_eq!(full.position(silent), None);
    assert_eq!(w.plan(2).start, vec![answered]);
}
