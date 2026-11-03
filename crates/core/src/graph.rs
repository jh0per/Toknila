//! The chain graph: tickets and release milestones joined by dependency edges.
//!
//! The graph decides what may start: [`ChainView::next_actions`] lists what should happen now
//! (a Backlog ticket became Ready, a Ready ticket autostarts, an auto milestone is cut), and
//! [`ChainView::settle`] applies those actions until nothing changes. Chain status, progress and
//! "where it stopped" are computed by [`ChainView::summary`] and never stored.
//!
//! Releases follow gitflow: a milestone is `planned`, then `cut` (a `release/<version>` branch
//! leaves `develop`), then `released`. Tickets after a milestone wait only for the cut; an
//! `after_release` edge makes them wait for the release itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::machine::{self, Effect, TicketEvent, TransitionError};
use crate::{
    BlockedReason, Chain, Edge, EdgeKind, NodeRef, Release, ReleaseChannel, ReleaseId,
    ReleaseState, StartMode, StatusReason, Ticket, TicketId, TicketKey, TicketStatus, Timestamp,
};

/// Why the graph or a graph operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("edge refers to a node that is not in this chain: {0:?}")]
    UnknownNode(NodeRef),
    #[error("a node cannot depend on itself: {0:?}")]
    SelfLoop(NodeRef),
    #[error("duplicate edge {from:?} -> {to:?}")]
    DuplicateEdge { from: NodeRef, to: NodeRef },
    #[error("after_release edges must start at a release milestone, not {0:?}")]
    AfterReleaseFromTicket(NodeRef),
    #[error("dependency cycle: {}", format_cycle(.0))]
    Cycle(Vec<String>),
    #[error("unknown ticket {0}")]
    UnknownTicket(TicketId),
    #[error("unknown release {0}")]
    UnknownRelease(ReleaseId),
    #[error("release {version} is {state:?}; cannot {action}")]
    ReleaseState {
        version: String,
        state: ReleaseState,
        action: &'static str,
    },
    #[error("release {version} still waits for: {}", .waiting_for.join(", "))]
    ReleaseNotReady {
        version: String,
        waiting_for: Vec<String>,
    },
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error("graph did not settle after {0} steps")]
    NoFixpoint(usize),
}

fn format_cycle(path: &[String]) -> String {
    path.join(" -> ")
}

/// Something the graph wants done.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Apply this event to the ticket through the state machine.
    Ticket {
        ticket: TicketId,
        event: TicketEvent,
    },
    /// Cut the milestone (auto mode, everything before it is done).
    CutRelease { release: ReleaseId },
}

/// One applied step, for the event log.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub action: Action,
    pub effects: Vec<Effect>,
}

/// Everything the graph rules need about one chain, as plain data.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainView {
    pub chain: Chain,
    pub tickets: Vec<Ticket>,
    pub releases: Vec<Release>,
    pub edges: Vec<Edge>,
    /// Which tickets went into which release; filled when a milestone is cut.
    pub release_tickets: Vec<(ReleaseId, TicketId)>,
}

/// Computed chain status (never stored).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainStatus {
    /// No tickets yet.
    Empty,
    /// Nothing started yet.
    NotStarted,
    /// Some ticket is Blocked.
    Blocked,
    /// Some ticket waits on a person (Waiting or Review).
    NeedsHuman,
    /// Agents are working.
    Running,
    /// Nothing runs and nothing waits on a person, but work remains (paused or waiting on a
    /// manual milestone).
    Idle,
    /// Every ticket is done and every milestone released.
    Done,
}

/// Where the chain stopped: the first ticket, in graph order, that waits on a person.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoppedAt {
    pub ticket: TicketKey,
    pub status: TicketStatus,
    pub reason: Option<StatusReason>,
    pub since: Timestamp,
    pub waiting_ms: u64,
}

/// The next milestone on the graph.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextRelease {
    pub release: ReleaseId,
    pub version: String,
    pub channel: ReleaseChannel,
    pub state: ReleaseState,
}

/// What a product person sees at a glance.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainSummary {
    pub status: ChainStatus,
    pub total: usize,
    /// Merged, Released or Cancelled (Definition of Done: in the work branch).
    pub done: usize,
    pub released: usize,
    pub stopped_at: Option<StoppedAt>,
    pub next_release: Option<NextRelease>,
}

const MAX_SETTLE_STEPS: usize = 10_000;

impl ChainView {
    /// Checks edges: known nodes, no self-loops or duplicates, `after_release` only from a
    /// milestone, and no cycles (the error names the cycle).
    pub fn validate(&self) -> Result<(), GraphError> {
        let nodes = self.nodes();
        let mut seen = BTreeSet::new();
        for e in &self.edges {
            for n in [e.from, e.to] {
                if !nodes.contains(&n) {
                    return Err(GraphError::UnknownNode(n));
                }
            }
            if e.from == e.to {
                return Err(GraphError::SelfLoop(e.from));
            }
            if !seen.insert((e.from, e.to)) {
                return Err(GraphError::DuplicateEdge {
                    from: e.from,
                    to: e.to,
                });
            }
            if e.kind == EdgeKind::AfterRelease && matches!(e.from, NodeRef::Ticket(_)) {
                return Err(GraphError::AfterReleaseFromTicket(e.from));
            }
        }
        if let Some(cycle) = self.find_cycle() {
            return Err(GraphError::Cycle(
                cycle.iter().map(|n| self.label(*n)).collect(),
            ));
        }
        Ok(())
    }

    /// Adds an edge if the graph stays valid; otherwise leaves the view unchanged.
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), GraphError> {
        self.edges.push(edge);
        if let Err(e) = self.validate() {
            self.edges.pop();
            return Err(e);
        }
        Ok(())
    }

    /// Human label of a node: ticket key or `release <version>`.
    pub fn label(&self, node: NodeRef) -> String {
        match node {
            NodeRef::Ticket(id) => self
                .ticket(id)
                .map(|t| t.key.to_string())
                .unwrap_or_else(|| id.to_string()),
            NodeRef::Release(id) => self
                .release(id)
                .map(|r| format!("release {}", r.version))
                .unwrap_or_else(|| id.to_string()),
        }
    }

    fn nodes(&self) -> BTreeSet<NodeRef> {
        let tickets = self.tickets.iter().map(|t| NodeRef::Ticket(t.id));
        let releases = self.releases.iter().map(|r| NodeRef::Release(r.id));
        tickets.chain(releases).collect()
    }

    fn ticket(&self, id: TicketId) -> Option<&Ticket> {
        self.tickets.iter().find(|t| t.id == id)
    }

    fn release(&self, id: ReleaseId) -> Option<&Release> {
        self.releases.iter().find(|r| r.id == id)
    }

    /// Stable order for nodes: tickets by number, then milestones by version.
    fn sort_key(&self, n: NodeRef) -> (u8, u32, String) {
        match n {
            NodeRef::Ticket(id) => (
                0,
                self.ticket(id).map_or(0, |t| t.key.number()),
                String::new(),
            ),
            NodeRef::Release(id) => (
                1,
                0,
                self.release(id)
                    .map(|r| r.version.clone())
                    .unwrap_or_default(),
            ),
        }
    }

    fn successors(&self) -> BTreeMap<NodeRef, Vec<NodeRef>> {
        let mut out: BTreeMap<NodeRef, Vec<NodeRef>> = BTreeMap::new();
        for e in &self.edges {
            out.entry(e.from).or_default().push(e.to);
        }
        out
    }

    fn find_cycle(&self) -> Option<Vec<NodeRef>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Visiting,
            Done,
        }
        fn visit(
            n: NodeRef,
            succ: &BTreeMap<NodeRef, Vec<NodeRef>>,
            marks: &mut HashMap<NodeRef, Mark>,
            stack: &mut Vec<NodeRef>,
        ) -> Option<Vec<NodeRef>> {
            match marks.get(&n) {
                Some(Mark::Done) => return None,
                Some(Mark::Visiting) => {
                    let start = stack.iter().position(|s| *s == n).unwrap_or(0);
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(n);
                    return Some(cycle);
                }
                None => {}
            }
            marks.insert(n, Mark::Visiting);
            stack.push(n);
            for next in succ.get(&n).into_iter().flatten() {
                if let Some(c) = visit(*next, succ, marks, stack) {
                    return Some(c);
                }
            }
            stack.pop();
            marks.insert(n, Mark::Done);
            None
        }
        let succ = self.successors();
        let mut marks = HashMap::new();
        // ULIDs minted in the same millisecond are unordered; walk in key order so the
        // reported cycle is the same on every run.
        let mut nodes: Vec<NodeRef> = self.nodes().into_iter().collect();
        nodes.sort_by_key(|n| self.sort_key(*n));
        for n in nodes {
            let mut stack = Vec::new();
            if let Some(c) = visit(n, &succ, &mut marks, &mut stack) {
                return Some(c);
            }
        }
        None
    }

    /// Nodes in dependency order; ties broken by ticket number, then milestone version.
    pub fn topo_order(&self) -> Vec<NodeRef> {
        let mut indegree: BTreeMap<NodeRef, usize> =
            self.nodes().into_iter().map(|n| (n, 0)).collect();
        for e in &self.edges {
            if let Some(d) = indegree.get_mut(&e.to) {
                *d += 1;
            }
        }
        let succ = self.successors();
        let mut ready: Vec<NodeRef> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut order = Vec::new();
        while !ready.is_empty() {
            ready.sort_by_key(|n| std::cmp::Reverse(self.sort_key(*n)));
            let Some(n) = ready.pop() else { break };
            order.push(n);
            for next in succ.get(&n).into_iter().flatten() {
                if let Some(d) = indegree.get_mut(next) {
                    *d -= 1;
                    if *d == 0 {
                        ready.push(*next);
                    }
                }
            }
        }
        order
    }

    /// Is the dependency `edge.from` satisfied for its successor?
    fn edge_satisfied(&self, edge: &Edge) -> bool {
        match edge.from {
            NodeRef::Ticket(id) => self.ticket(id).is_some_and(|t| {
                matches!(
                    t.status,
                    TicketStatus::Merged | TicketStatus::Released | TicketStatus::Cancelled
                )
            }),
            NodeRef::Release(id) => self.release(id).is_some_and(|r| match edge.kind {
                EdgeKind::Normal => matches!(r.state, ReleaseState::Cut | ReleaseState::Released),
                EdgeKind::AfterRelease => r.state == ReleaseState::Released,
            }),
        }
    }

    fn unsatisfied_deps(&self, node: NodeRef) -> Vec<NodeRef> {
        self.edges
            .iter()
            .filter(|e| e.to == node && !self.edge_satisfied(e))
            .map(|e| e.from)
            .collect()
    }

    /// What should happen now, in graph order. Empty when the chain is settled.
    pub fn next_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for node in self.topo_order() {
            match node {
                NodeRef::Ticket(id) => {
                    let Some(t) = self.ticket(id) else { continue };
                    let deps_ok = self.unsatisfied_deps(node).is_empty();
                    match t.status {
                        TicketStatus::Backlog if deps_ok && !t.paused => {
                            actions.push(Action::Ticket {
                                ticket: id,
                                event: TicketEvent::DepsSatisfied,
                            })
                        }
                        TicketStatus::Ready if !deps_ok => actions.push(Action::Ticket {
                            ticket: id,
                            event: TicketEvent::DepsUnsatisfied,
                        }),
                        TicketStatus::Ready if self.chain.autostart => {
                            actions.push(Action::Ticket {
                                ticket: id,
                                event: TicketEvent::Start,
                            })
                        }
                        _ => {}
                    }
                }
                NodeRef::Release(id) => {
                    let Some(r) = self.release(id) else { continue };
                    if r.state == ReleaseState::Planned
                        && r.start_mode == StartMode::Auto
                        && self.unsatisfied_deps(node).is_empty()
                    {
                        actions.push(Action::CutRelease { release: id });
                    }
                }
            }
        }
        actions
    }

    /// Applies one action. Ticket events go through the state machine.
    pub fn apply_action(
        &mut self,
        action: &Action,
        now: Timestamp,
    ) -> Result<Vec<Effect>, GraphError> {
        match action {
            Action::Ticket { ticket, event } => self.apply_ticket_event(*ticket, event, now),
            Action::CutRelease { release } => {
                self.cut_release(*release, now)?;
                Ok(Vec::new())
            }
        }
    }

    /// Applies an event to one ticket and then lets the graph react to its effects
    /// (a reopened dependency blocks running dependents and sends Ready ones back).
    pub fn apply_ticket_event(
        &mut self,
        ticket: TicketId,
        event: &TicketEvent,
        now: Timestamp,
    ) -> Result<Vec<Effect>, GraphError> {
        let idx = self
            .tickets
            .iter()
            .position(|t| t.id == ticket)
            .ok_or(GraphError::UnknownTicket(ticket))?;
        let tr = machine::apply(&self.tickets[idx], event, now)?;
        let key = tr.ticket.key.clone();
        self.tickets[idx] = tr.ticket;
        if tr.effects.contains(&Effect::DependencyReopened) {
            self.reopen_dependents(ticket, &key, now)?;
        }
        Ok(tr.effects)
    }

    fn reopen_dependents(
        &mut self,
        ticket: TicketId,
        key: &TicketKey,
        now: Timestamp,
    ) -> Result<(), GraphError> {
        let dependents: Vec<TicketId> = self
            .edges
            .iter()
            .filter(|e| e.from == NodeRef::Ticket(ticket))
            .filter_map(|e| match e.to {
                NodeRef::Ticket(id) => Some(id),
                NodeRef::Release(_) => None,
            })
            .collect();
        for id in dependents {
            let Some(t) = self.ticket(id) else { continue };
            let event = match t.status {
                TicketStatus::Running | TicketStatus::Waiting => TicketEvent::Block {
                    reason: BlockedReason::DependencyReopened {
                        dependency: key.clone(),
                    },
                },
                TicketStatus::Ready => TicketEvent::DepsUnsatisfied,
                _ => continue,
            };
            self.apply_ticket_event(id, &event, now)?;
        }
        Ok(())
    }

    /// Applies [`Self::next_actions`] until there are none. Returns every applied step.
    pub fn settle(&mut self, now: Timestamp) -> Result<Vec<Applied>, GraphError> {
        let mut log = Vec::new();
        loop {
            let Some(action) = self.next_actions().into_iter().next() else {
                return Ok(log);
            };
            let effects = self.apply_action(&action, now)?;
            log.push(Applied { action, effects });
            if log.len() >= MAX_SETTLE_STEPS {
                return Err(GraphError::NoFixpoint(log.len()));
            }
        }
    }

    /// Cuts a milestone: auto mode calls this from [`Self::settle`], manual mode from
    /// `tok release start`. Everything before it must be done. Records which tickets go in:
    /// the done tickets upstream of the milestone that no earlier milestone took.
    pub fn cut_release(&mut self, release: ReleaseId, now: Timestamp) -> Result<(), GraphError> {
        let r = self
            .release(release)
            .ok_or(GraphError::UnknownRelease(release))?;
        if r.state != ReleaseState::Planned {
            return Err(GraphError::ReleaseState {
                version: r.version.clone(),
                state: r.state,
                action: "cut",
            });
        }
        let waiting = self.unsatisfied_deps(NodeRef::Release(release));
        if !waiting.is_empty() {
            return Err(GraphError::ReleaseNotReady {
                version: r.version.clone(),
                waiting_for: waiting.into_iter().map(|n| self.label(n)).collect(),
            });
        }
        let taken: BTreeSet<TicketId> = self.release_tickets.iter().map(|(_, t)| *t).collect();
        let members: Vec<TicketId> = self
            .ancestors(NodeRef::Release(release))
            .into_iter()
            .filter_map(|n| match n {
                NodeRef::Ticket(id) => Some(id),
                NodeRef::Release(_) => None,
            })
            .filter(|id| !taken.contains(id))
            .filter(|id| {
                self.ticket(*id)
                    .is_some_and(|t| t.status == TicketStatus::Merged)
            })
            .collect();
        self.release_tickets
            .extend(members.into_iter().map(|t| (release, t)));
        if let Some(r) = self.releases.iter_mut().find(|r| r.id == release) {
            r.state = ReleaseState::Cut;
            r.cut_at = Some(now);
        }
        Ok(())
    }

    /// Marks a cut milestone released and moves its tickets from Merged to Released.
    pub fn complete_release(
        &mut self,
        release: ReleaseId,
        now: Timestamp,
    ) -> Result<Vec<Applied>, GraphError> {
        let r = self
            .release(release)
            .ok_or(GraphError::UnknownRelease(release))?;
        if r.state != ReleaseState::Cut {
            return Err(GraphError::ReleaseState {
                version: r.version.clone(),
                state: r.state,
                action: "release",
            });
        }
        if let Some(r) = self.releases.iter_mut().find(|r| r.id == release) {
            r.state = ReleaseState::Released;
            r.released_at = Some(now);
        }
        let members: Vec<TicketId> = self
            .release_tickets
            .iter()
            .filter(|(r, _)| *r == release)
            .map(|(_, t)| *t)
            .collect();
        let mut log = Vec::new();
        for id in members {
            if self
                .ticket(id)
                .is_some_and(|t| t.status == TicketStatus::Merged)
            {
                let action = Action::Ticket {
                    ticket: id,
                    event: TicketEvent::Released,
                };
                let effects = self.apply_action(&action, now)?;
                log.push(Applied { action, effects });
            }
        }
        Ok(log)
    }

    fn ancestors(&self, node: NodeRef) -> BTreeSet<NodeRef> {
        let mut out = BTreeSet::new();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            for e in self.edges.iter().filter(|e| e.to == n) {
                if out.insert(e.from) {
                    stack.push(e.from);
                }
            }
        }
        out
    }

    /// Status, progress, where it stopped and the next milestone, computed from tickets.
    pub fn summary(&self, now: Timestamp) -> ChainSummary {
        use TicketStatus as S;
        let total = self.tickets.len();
        let count = |f: &dyn Fn(S) -> bool| self.tickets.iter().filter(|t| f(t.status)).count();
        let done = count(&|s| matches!(s, S::Merged | S::Released | S::Cancelled));
        let released = count(&|s| s == S::Released);
        let any = |f: &dyn Fn(S) -> bool| self.tickets.iter().any(|t| f(t.status));

        let order = self.topo_order();
        let in_order = order.iter().filter_map(|n| match n {
            NodeRef::Ticket(id) => self.ticket(*id),
            NodeRef::Release(_) => None,
        });
        let stopped_at = in_order
            .clone()
            .find(|t| matches!(t.status, S::Waiting | S::Blocked))
            .or_else(|| in_order.clone().find(|t| t.status == S::Review))
            .map(|t| StoppedAt {
                ticket: t.key.clone(),
                status: t.status,
                reason: t.status_reason.clone(),
                since: t.status_since,
                waiting_ms: now.millis_since(t.status_since),
            });
        let next_release = order
            .iter()
            .filter_map(|n| match n {
                NodeRef::Release(id) => self.release(*id),
                NodeRef::Ticket(_) => None,
            })
            .find(|r| r.state != ReleaseState::Released)
            .map(|r| NextRelease {
                release: r.id,
                version: r.version.clone(),
                channel: r.channel,
                state: r.state,
            });

        let all_released = self
            .releases
            .iter()
            .all(|r| r.state == ReleaseState::Released);
        let status = if total == 0 {
            ChainStatus::Empty
        } else if done == total && all_released {
            ChainStatus::Done
        } else if any(&|s| s == S::Blocked) {
            ChainStatus::Blocked
        } else if any(&|s| matches!(s, S::Waiting | S::Review)) {
            ChainStatus::NeedsHuman
        } else if any(&|s| s == S::Running) {
            ChainStatus::Running
        } else if self
            .tickets
            .iter()
            .all(|t| t.status == S::Backlog && !t.paused)
            && self
                .releases
                .iter()
                .all(|r| r.state == ReleaseState::Planned)
        {
            ChainStatus::NotStarted
        } else {
            ChainStatus::Idle
        };
        ChainSummary {
            status,
            total,
            done,
            released,
            stopped_at,
            next_release,
        }
    }
}

#[cfg(test)]
mod tests;
