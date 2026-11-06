use super::*;
use crate::{ChainId, ProjectId, WaitingReason};

mod wildcard;

const T: Timestamp = Timestamp(1_000);

/// Settles the graph and starts every card that wants to run (no slot limit), as the
/// application does after each command; returns every step and event in order.
fn settle(v: &mut ChainView) -> Settled {
    let mut all = Settled::default();
    loop {
        let step = v.settle(T).expect("settle");
        all.applied.extend(step.applied);
        all.events.extend(step.events);
        let start = crate::schedule::plan(std::slice::from_ref(v), u32::MAX, &[]).start;
        if start.is_empty() {
            return all;
        }
        for ticket in start {
            let event = TicketEvent::Start;
            let (effects, events) = v
                .record(|v| v.apply_ticket_event(ticket, &event, T))
                .expect("start");
            let action = Action::Ticket { ticket, event };
            all.applied.push(Applied { action, effects });
            all.events.extend(events);
        }
    }
}

struct Builder {
    view: ChainView,
    next: u32,
}

impl Builder {
    fn new(autostart: bool) -> Self {
        let chain = Chain {
            id: ChainId::new(),
            project_id: ProjectId::new(),
            name: "demo".to_owned(),
            goal: String::new(),
            autostart,
            review_mode: None,
            reviewer: None,
        };
        let view = ChainView {
            chain,
            tickets: vec![],
            releases: vec![],
            edges: vec![],
            release_tickets: vec![],
        };
        Self { view, next: 1 }
    }

    fn ticket(&mut self) -> NodeRef {
        let t = Ticket {
            id: TicketId::new(),
            chain_id: self.view.chain.id,
            key: format!("DEMO-{}", self.next).parse().expect("key"),
            title: format!("t{}", self.next),
            prompt: String::new(),
            acceptance: vec![],
            status: TicketStatus::Backlog,
            status_reason: None,
            status_since: Timestamp(0),
            paused: false,
            queued: false,
            profile: Some("sim".to_owned()),
            review_mode: None,
            reviewer: None,
            branch: None,
            needs_secrets: false,
            tokens_used: 0,
            wildcard: None,
            done_out_of_order: None,
            review: crate::Review::default(),
        };
        self.next += 1;
        let id = t.id;
        self.view.tickets.push(t);
        NodeRef::Ticket(id)
    }

    fn release(&mut self, version: &str, start_mode: StartMode) -> NodeRef {
        let r = Release {
            id: ReleaseId::new(),
            chain_id: self.view.chain.id,
            version: version.to_owned(),
            channel: ReleaseChannel::TagOnly,
            start_mode,
            state: ReleaseState::Planned,
            branch: None,
            cut_at: None,
            released_at: None,
        };
        let id = r.id;
        self.view.releases.push(r);
        NodeRef::Release(id)
    }

    fn edge(&mut self, from: NodeRef, to: NodeRef) {
        self.edge_kind(from, to, EdgeKind::Normal);
    }

    fn edge_kind(&mut self, from: NodeRef, to: NodeRef, kind: EdgeKind) {
        let edge = Edge {
            chain_id: self.view.chain.id,
            from,
            to,
            kind,
        };
        self.view.add_edge(edge).expect("valid edge");
    }
}

fn tid(n: NodeRef) -> TicketId {
    match n {
        NodeRef::Ticket(id) => id,
        NodeRef::Release(_) => panic!("not a ticket"),
    }
}

fn rid(n: NodeRef) -> ReleaseId {
    match n {
        NodeRef::Release(id) => id,
        NodeRef::Ticket(_) => panic!("not a release"),
    }
}

fn status(v: &ChainView, n: NodeRef) -> TicketStatus {
    v.ticket(tid(n)).map(|t| t.status).expect("ticket")
}

fn rstate(v: &ChainView, n: NodeRef) -> ReleaseState {
    v.release(rid(n)).map(|r| r.state).expect("release")
}

fn event(v: &mut ChainView, n: NodeRef, e: TicketEvent) {
    v.apply_ticket_event(tid(n), &e, T)
        .unwrap_or_else(|err| panic!("{e:?}: {err}"));
    settle(v);
}

/// Running -> Review -> merging -> Merged, then let the graph react.
fn finish_and_accept(v: &mut ChainView, n: NodeRef) {
    event(
        v,
        n,
        TicketEvent::Finished {
            summary: "done".to_owned(),
            open_threads: vec![],
            mode: crate::ReviewMode::Human,
        },
    );
    event(v, n, TicketEvent::Accept { comment: None });
    event(v, n, TicketEvent::MergeDone);
}

#[test]
fn linear_chain_runs_one_after_another() {
    let mut b = Builder::new(true);
    let (a, bb, c) = (b.ticket(), b.ticket(), b.ticket());
    b.edge(a, bb);
    b.edge(bb, c);
    let mut v = b.view;
    settle(&mut v);
    assert_eq!(
        [status(&v, a), status(&v, bb), status(&v, c)],
        [
            TicketStatus::Running,
            TicketStatus::Backlog,
            TicketStatus::Backlog
        ]
    );
    finish_and_accept(&mut v, a);
    assert_eq!(
        [status(&v, bb), status(&v, c)],
        [TicketStatus::Running, TicketStatus::Backlog]
    );
    finish_and_accept(&mut v, bb);
    assert_eq!(status(&v, c), TicketStatus::Running);
    finish_and_accept(&mut v, c);
    assert_eq!(v.summary(T).status, ChainStatus::Done);
}

#[test]
fn parallel_roots_start_together() {
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    let mut v = b.view;
    settle(&mut v);
    assert_eq!(
        [status(&v, a), status(&v, c)],
        [TicketStatus::Running, TicketStatus::Running]
    );
}

#[test]
fn diamond_waits_for_both_branches() {
    let mut b = Builder::new(true);
    let (a, l, r, d) = (b.ticket(), b.ticket(), b.ticket(), b.ticket());
    b.edge(a, l);
    b.edge(a, r);
    b.edge(l, d);
    b.edge(r, d);
    let mut v = b.view;
    settle(&mut v);
    finish_and_accept(&mut v, a);
    assert_eq!(
        [status(&v, l), status(&v, r)],
        [TicketStatus::Running, TicketStatus::Running]
    );
    finish_and_accept(&mut v, l);
    assert_eq!(status(&v, d), TicketStatus::Backlog, "d still waits for r");
    finish_and_accept(&mut v, r);
    assert_eq!(status(&v, d), TicketStatus::Running);
}

#[test]
fn autostart_off_stops_at_ready() {
    let mut b = Builder::new(false);
    let a = b.ticket();
    let mut v = b.view;
    settle(&mut v);
    assert_eq!(status(&v, a), TicketStatus::Ready);
    event(&mut v, a, TicketEvent::Start);
    assert_eq!(status(&v, a), TicketStatus::Running);
}

#[test]
fn cycle_is_rejected_with_its_path() {
    let mut b = Builder::new(true);
    let (a, bb, c) = (b.ticket(), b.ticket(), b.ticket());
    b.edge(a, bb);
    b.edge(bb, c);
    let before = b.view.clone();
    let err = b.view.add_edge(Edge {
        chain_id: b.view.chain.id,
        from: c,
        to: a,
        kind: EdgeKind::Normal,
    });
    assert_eq!(
        err,
        Err(GraphError::Cycle(vec![
            "DEMO-1".into(),
            "DEMO-2".into(),
            "DEMO-3".into(),
            "DEMO-1".into()
        ]))
    );
    assert_eq!(
        err.expect_err("cycle").to_string(),
        "dependency cycle: DEMO-1 -> DEMO-2 -> DEMO-3 -> DEMO-1"
    );
    assert_eq!(b.view, before, "a rejected edge leaves the graph unchanged");
}

#[test]
fn bad_edges_are_rejected() {
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    let chain_id = b.view.chain.id;
    let e = |from, to, kind| Edge {
        chain_id,
        from,
        to,
        kind,
    };
    assert_eq!(
        b.view.add_edge(e(a, a, EdgeKind::Normal)),
        Err(GraphError::SelfLoop(a))
    );
    b.edge(a, c);
    assert_eq!(
        b.view.add_edge(e(a, c, EdgeKind::Normal)),
        Err(GraphError::DuplicateEdge { from: a, to: c })
    );
    let ghost = NodeRef::Ticket(TicketId::new());
    assert_eq!(
        b.view.add_edge(e(ghost, c, EdgeKind::Normal)),
        Err(GraphError::UnknownNode(ghost))
    );
    let d = b.ticket();
    assert_eq!(
        b.view.add_edge(e(c, d, EdgeKind::AfterRelease)),
        Err(GraphError::AfterReleaseFromTicket(c))
    );
}

/// The phase-0 gate chain: A -> B -> R1 (manual, tag only) -> C, and D after A, before R1.
#[test]
fn gate_chain_with_manual_release() {
    let mut b = Builder::new(true);
    let (a, bb, c, d) = (b.ticket(), b.ticket(), b.ticket(), b.ticket());
    let r1 = b.release("0.1.0", StartMode::Manual);
    b.edge(a, bb);
    b.edge(bb, r1);
    b.edge(r1, c);
    b.edge(a, d);
    b.edge(d, r1);
    let mut v = b.view;
    settle(&mut v);
    assert_eq!(status(&v, a), TicketStatus::Running);
    assert_eq!(v.summary(T).status, ChainStatus::Running);

    // A asks, gets an answer, finishes, is returned, finishes again, is accepted.
    let q = WaitingReason::Question {
        text: "який фреймворк?".to_owned(),
    };
    event(&mut v, a, TicketEvent::AskHuman { reason: q });
    let s = v.summary(Timestamp(4_000));
    assert_eq!(s.status, ChainStatus::NeedsHuman);
    assert_eq!(
        s.stopped_at
            .as_ref()
            .map(|x| (x.ticket.to_string(), x.waiting_ms)),
        Some(("DEMO-1".into(), 3_000))
    );
    event(&mut v, a, TicketEvent::InstructionDelivered);
    event(
        &mut v,
        a,
        TicketEvent::Finished {
            summary: "зроблено X".to_owned(),
            open_threads: vec![],
            mode: crate::ReviewMode::Human,
        },
    );
    event(
        &mut v,
        a,
        TicketEvent::Return {
            comment: "додай тест".to_owned(),
        },
    );
    finish_and_accept(&mut v, a);
    assert_eq!(
        [status(&v, bb), status(&v, d)],
        [TicketStatus::Running, TicketStatus::Running]
    );

    // B blocks: the chain names it as where it stopped.
    event(
        &mut v,
        bb,
        TicketEvent::Block {
            reason: BlockedReason::Manual {
                text: "чекаємо ключ API".to_owned(),
            },
        },
    );
    let s = v.summary(T);
    assert_eq!(s.status, ChainStatus::Blocked);
    assert_eq!(
        s.stopped_at.map(|x| x.ticket.to_string()),
        Some("DEMO-2".into())
    );
    event(&mut v, bb, TicketEvent::Unblock);
    assert_eq!(status(&v, bb), TicketStatus::Running);

    // Manual release refuses to cut before B and D are merged.
    let err = v.cut_release(rid(r1), T).expect_err("not ready");
    assert!(
        matches!(err, GraphError::ReleaseNotReady { ref waiting_for, .. } if waiting_for == &["DEMO-2", "DEMO-4"]),
        "{err}"
    );

    finish_and_accept(&mut v, bb);
    finish_and_accept(&mut v, d);
    assert_eq!(
        rstate(&v, r1),
        ReleaseState::Planned,
        "manual milestone waits for a person"
    );
    assert_eq!(status(&v, c), TicketStatus::Backlog);
    assert_eq!(v.summary(T).status, ChainStatus::Idle);
    assert_eq!(
        v.summary(T).next_release.map(|r| r.state),
        Some(ReleaseState::Planned)
    );

    // Cut: C starts right away; the release does not hold development.
    v.cut_release(rid(r1), T).expect("cut");
    settle(&mut v);
    assert_eq!(rstate(&v, r1), ReleaseState::Cut);
    assert_eq!(status(&v, c), TicketStatus::Running);

    // Released: A, B, D become Released; C goes into the next release.
    v.complete_release(rid(r1), T).expect("release");
    assert_eq!(
        [status(&v, a), status(&v, bb), status(&v, d)],
        [TicketStatus::Released; 3]
    );
    assert_eq!(status(&v, c), TicketStatus::Running);
    let s = v.summary(T);
    assert_eq!((s.total, s.done, s.released), (4, 3, 3));
    assert_eq!(s.next_release, None);
}

#[test]
fn auto_release_cuts_itself() {
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    let r = b.release("1.0.0", StartMode::Auto);
    b.edge(a, r);
    b.edge(r, c);
    let mut v = b.view;
    settle(&mut v);
    finish_and_accept(&mut v, a);
    assert_eq!(rstate(&v, r), ReleaseState::Cut);
    assert_eq!(status(&v, c), TicketStatus::Running);
    assert_eq!(v.release_tickets, vec![(rid(r), tid(a))]);
}

#[test]
fn after_release_edge_waits_for_released() {
    let mut b = Builder::new(true);
    let a = b.ticket();
    let r = b.release("1.0.0", StartMode::Auto);
    let c = b.ticket();
    b.edge(a, r);
    b.edge_kind(r, c, EdgeKind::AfterRelease);
    let mut v = b.view;
    settle(&mut v);
    finish_and_accept(&mut v, a);
    assert_eq!(rstate(&v, r), ReleaseState::Cut);
    assert_eq!(
        status(&v, c),
        TicketStatus::Backlog,
        "after_release waits for the store"
    );
    v.complete_release(rid(r), T).expect("release");
    settle(&mut v);
    assert_eq!(status(&v, c), TicketStatus::Running);
}

#[test]
fn second_release_takes_only_new_tickets() {
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    let r1 = b.release("1.0.0", StartMode::Auto);
    let r2 = b.release("1.1.0", StartMode::Auto);
    b.edge(a, r1);
    b.edge(r1, c);
    b.edge(c, r2);
    let mut v = b.view;
    settle(&mut v);
    finish_and_accept(&mut v, a);
    finish_and_accept(&mut v, c);
    assert_eq!(
        v.release_tickets,
        vec![(rid(r1), tid(a)), (rid(r2), tid(c))]
    );
}

#[test]
fn reopen_blocks_running_dependents_and_unreadies_ready_ones() {
    let mut b = Builder::new(false);
    let (a, run, rdy) = (b.ticket(), b.ticket(), b.ticket());
    b.edge(a, run);
    b.edge(a, rdy);
    let mut v = b.view;
    settle(&mut v);
    event(&mut v, a, TicketEvent::Start);
    finish_and_accept(&mut v, a);
    event(&mut v, run, TicketEvent::Start);
    assert_eq!(
        [status(&v, run), status(&v, rdy)],
        [TicketStatus::Running, TicketStatus::Ready]
    );

    event(&mut v, a, TicketEvent::Reopen);
    assert_eq!(status(&v, a), TicketStatus::Review);
    assert_eq!(status(&v, run), TicketStatus::Blocked);
    assert_eq!(status(&v, rdy), TicketStatus::Backlog);
    let reason = v.ticket(tid(run)).and_then(|t| t.status_reason.clone());
    assert!(matches!(
        reason,
        Some(StatusReason::Blocked { reason: BlockedReason::DependencyReopened { ref dependency }, .. })
            if dependency.to_string() == "DEMO-1"
    ));
}

#[test]
fn cancelled_dependency_counts_as_satisfied() {
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    b.edge(a, c);
    let mut v = b.view;
    settle(&mut v);
    event(
        &mut v,
        a,
        TicketEvent::Cancel {
            reason: "won't do".to_owned(),
        },
    );
    assert_eq!(status(&v, c), TicketStatus::Running);
}

#[test]
fn paused_ticket_is_skipped_until_resumed() {
    let mut b = Builder::new(true);
    let a = b.ticket();
    let mut v = b.view;
    event(&mut v, a, TicketEvent::Pause);
    assert_eq!(status(&v, a), TicketStatus::Backlog);
    assert!(v.next_actions().is_empty());
    assert_eq!(v.summary(T).status, ChainStatus::Idle);
    event(&mut v, a, TicketEvent::Resume);
    assert_eq!(status(&v, a), TicketStatus::Running);
}

#[test]
fn empty_and_not_started_chains() {
    let v = Builder::new(true).view;
    assert_eq!(v.summary(T).status, ChainStatus::Empty);
    let mut b = Builder::new(true);
    b.ticket();
    assert_eq!(b.view.summary(T).status, ChainStatus::NotStarted);
}

#[test]
fn chain_view_json_roundtrips() {
    let mut b = Builder::new(true);
    let a = b.ticket();
    let r = b.release("1.0.0", StartMode::Manual);
    b.edge(a, r);
    let json = serde_json::to_string(&b.view).expect("serialize");
    let back: ChainView = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, b.view);
}

#[test]
fn settle_journals_every_intermediate_transition() {
    use crate::events::DomainEvent;
    let mut b = Builder::new(true);
    let (a, c) = (b.ticket(), b.ticket());
    b.edge(a, c);
    let mut v = b.view;
    let settled = settle(&mut v);
    let moves: Vec<(String, TicketStatus, TicketStatus)> = settled
        .events
        .iter()
        .map(|e| match e {
            DomainEvent::TicketStatusChanged { key, from, to, .. } => (key.to_string(), *from, *to),
            other => panic!("unexpected event {other:?}"),
        })
        .collect();
    assert_eq!(
        moves,
        [
            (
                "DEMO-1".to_owned(),
                TicketStatus::Backlog,
                TicketStatus::Ready
            ),
            (
                "DEMO-1".to_owned(),
                TicketStatus::Ready,
                TicketStatus::Running
            ),
        ]
    );
    assert_eq!(settled.applied.len(), 2);
    assert_eq!(v.settle(T).expect("settle again").events, []);
}
