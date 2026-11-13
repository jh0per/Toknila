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
fn a_live_worker_and_reviewer_of_a_card_in_review_each_hold_a_slot() {
    let mut w = World::new();
    let c = w.chain(false);
    let reviewed = w.card(c, TicketStatus::Review, false);
    let running = w.card(c, TicketStatus::Running, false);
    let third = w.card(c, TicketStatus::Ready, true);
    // Without runners a card in review holds nothing: one slot used, the third starts.
    let none = plan(&w.chains, 2, &[]);
    assert_eq!((none.used, none.start.clone()), (1, vec![third]));
    // With its worker and its reviewer alive the card holds two; with the running card the
    // limit of 2 is passed and the third stays queued.
    let live = [
        (reviewed, AgentRole::Worker),
        (reviewed, AgentRole::Reviewer),
    ];
    let p = plan(&w.chains, 2, &live);
    assert_eq!((p.used, p.start, p.queue), (3, vec![], vec![third]));
    // A running card's live worker is the same slot, counted once; a stranger's runner is not ours.
    let same = [
        (running, AgentRole::Worker),
        (TicketId::new(), AgentRole::Worker),
    ];
    assert_eq!(plan(&w.chains, 5, &same).used, 1);
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
