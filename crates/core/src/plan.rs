//! Chain files: TOML import and export of a chain (`tok chain import|export`).
//!
//! The database is the source of truth; a file is only a way in and out. Inside a file, nodes
//! have local ids (`A`, `R1`, or the ticket keys on export) that `after` lists refer to.
//!
//! ```toml
//! [chain]
//! name = "demo"
//! goal = "Phase-0 gate"
//!
//! [[ticket]]
//! id = "A"
//! title = "A"
//! subtasks = ["write the test"]
//!
//! [[ticket]]
//! id = "B"
//! after = ["A"]
//!
//! [[release]]
//! id = "R1"            # version defaults to the id
//! start = "manual"     # or "auto"
//! channel = "tag_only"
//! after = ["B"]
//! ```

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use toml::Spanned;

use crate::graph::ChainView;
use crate::{
    Chain, ChainId, Edge, EdgeKind, NodeRef, ProjectId, Release, ReleaseChannel, ReleaseId,
    ReleaseState, ReviewMode, StartMode, Ticket, TicketId, TicketKey, TicketStatus, Timestamp,
};

/// A problem in a chain file. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct PlanError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    chain: RawChain,
    #[serde(default)]
    ticket: Vec<RawTicket>,
    #[serde(default)]
    release: Vec<RawRelease>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChain {
    name: String,
    #[serde(default)]
    goal: String,
    autostart: Option<bool>,
    review_mode: Option<ReviewMode>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTicket {
    id: Spanned<String>,
    title: Option<String>,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    acceptance: Vec<String>,
    #[serde(default)]
    subtasks: Vec<String>,
    #[serde(default)]
    after: Vec<Spanned<String>>,
    #[serde(default)]
    after_release: Vec<Spanned<String>>,
    profile: Option<String>,
    review_mode: Option<ReviewMode>,
    #[serde(default)]
    needs_secrets: bool,
    #[serde(default)]
    paused: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRelease {
    id: Spanned<String>,
    version: Option<String>,
    channel: Option<ReleaseChannel>,
    start: Option<StartMode>,
    #[serde(default)]
    after: Vec<Spanned<String>>,
    #[serde(default)]
    before: Vec<Spanned<String>>,
}

/// Chain settings from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanChain {
    pub name: String,
    pub goal: String,
    pub autostart: bool,
    pub review_mode: Option<ReviewMode>,
}

/// One ticket from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanTicket {
    pub local_id: String,
    pub title: String,
    pub prompt: String,
    pub acceptance: Vec<String>,
    pub subtasks: Vec<String>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub needs_secrets: bool,
    pub paused: bool,
}

/// One milestone from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRelease {
    pub local_id: String,
    pub version: String,
    pub channel: ReleaseChannel,
    pub start_mode: StartMode,
}

/// A node of a plan: index into `tickets` or `releases`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PlanNode {
    Ticket(usize),
    Release(usize),
}

/// A dependency in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PlanEdge {
    pub from: PlanNode,
    pub to: PlanNode,
    pub kind: EdgeKind,
}

/// A validated chain file: unique ids, known references, no cycles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub chain: PlanChain,
    pub tickets: Vec<PlanTicket>,
    pub releases: Vec<PlanRelease>,
    pub edges: Vec<PlanEdge>,
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// Parses and validates a chain file.
pub fn parse(text: &str) -> Result<Plan, PlanError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| PlanError {
        line: e.span().map_or(1, |s| line_of(text, s.start)),
        message: e.message().to_owned(),
    })?;
    let err = |span: std::ops::Range<usize>, message: String| PlanError {
        line: line_of(text, span.start),
        message,
    };

    let mut ids: HashMap<String, PlanNode> = HashMap::new();
    for (i, t) in raw.ticket.iter().enumerate() {
        if t.id.get_ref().trim().is_empty() {
            return Err(err(t.id.span(), "ticket id is empty".into()));
        }
        if ids
            .insert(t.id.get_ref().clone(), PlanNode::Ticket(i))
            .is_some()
        {
            return Err(err(
                t.id.span(),
                format!("duplicate id {:?}", t.id.get_ref()),
            ));
        }
    }
    for (i, r) in raw.release.iter().enumerate() {
        if ids
            .insert(r.id.get_ref().clone(), PlanNode::Release(i))
            .is_some()
        {
            return Err(err(
                r.id.span(),
                format!("duplicate id {:?}", r.id.get_ref()),
            ));
        }
    }
    let node = |s: &Spanned<String>| {
        ids.get(s.get_ref())
            .copied()
            .ok_or_else(|| err(s.span(), format!("unknown id {:?}", s.get_ref())))
    };

    let mut edges = Vec::new();
    for (i, t) in raw.ticket.iter().enumerate() {
        for a in &t.after {
            edges.push(PlanEdge {
                from: node(a)?,
                to: PlanNode::Ticket(i),
                kind: EdgeKind::Normal,
            });
        }
        for a in &t.after_release {
            let from = node(a)?;
            if !matches!(from, PlanNode::Release(_)) {
                return Err(err(
                    a.span(),
                    format!(
                        "after_release needs a release, {:?} is a ticket",
                        a.get_ref()
                    ),
                ));
            }
            edges.push(PlanEdge {
                from,
                to: PlanNode::Ticket(i),
                kind: EdgeKind::AfterRelease,
            });
        }
    }
    for (i, r) in raw.release.iter().enumerate() {
        for a in &r.after {
            edges.push(PlanEdge {
                from: node(a)?,
                to: PlanNode::Release(i),
                kind: EdgeKind::Normal,
            });
        }
        for b in &r.before {
            edges.push(PlanEdge {
                from: PlanNode::Release(i),
                to: node(b)?,
                kind: EdgeKind::Normal,
            });
        }
    }

    let name_of = |n: PlanNode| match n {
        PlanNode::Ticket(i) => raw.ticket[i].id.get_ref().clone(),
        PlanNode::Release(i) => raw.release[i].id.get_ref().clone(),
    };
    let line_of_node = |n: PlanNode| match n {
        PlanNode::Ticket(i) => line_of(text, raw.ticket[i].id.span().start),
        PlanNode::Release(i) => line_of(text, raw.release[i].id.span().start),
    };
    for (k, e) in edges.iter().enumerate() {
        if e.from == e.to {
            return Err(PlanError {
                line: line_of_node(e.to),
                message: format!("{} depends on itself", name_of(e.to)),
            });
        }
        if edges[..k].iter().any(|x| x.from == e.from && x.to == e.to) {
            return Err(PlanError {
                line: line_of_node(e.to),
                message: format!("{} -> {} is listed twice", name_of(e.from), name_of(e.to)),
            });
        }
    }
    if let Some(cycle) = find_cycle(&edges, raw.ticket.len(), raw.release.len()) {
        return Err(PlanError {
            line: line_of_node(cycle[0]),
            message: format!(
                "dependency cycle: {}",
                cycle
                    .iter()
                    .map(|n| name_of(*n))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        });
    }

    Ok(Plan {
        chain: PlanChain {
            name: raw.chain.name,
            goal: raw.chain.goal,
            autostart: raw.chain.autostart.unwrap_or(true),
            review_mode: raw.chain.review_mode,
        },
        tickets: raw
            .ticket
            .into_iter()
            .map(|t| PlanTicket {
                title: t.title.unwrap_or_else(|| t.id.get_ref().clone()),
                local_id: t.id.into_inner(),
                prompt: t.prompt,
                acceptance: t.acceptance,
                subtasks: t.subtasks,
                profile: t.profile,
                review_mode: t.review_mode,
                needs_secrets: t.needs_secrets,
                paused: t.paused,
            })
            .collect(),
        releases: raw
            .release
            .into_iter()
            .map(|r| PlanRelease {
                version: r.version.unwrap_or_else(|| r.id.get_ref().clone()),
                local_id: r.id.into_inner(),
                channel: r.channel.unwrap_or(ReleaseChannel::TagOnly),
                start_mode: r.start.unwrap_or(StartMode::Manual),
            })
            .collect(),
        edges,
    })
}

fn find_cycle(edges: &[PlanEdge], tickets: usize, releases: usize) -> Option<Vec<PlanNode>> {
    fn visit(
        n: PlanNode,
        edges: &[PlanEdge],
        state: &mut HashMap<PlanNode, bool>,
        stack: &mut Vec<PlanNode>,
    ) -> Option<Vec<PlanNode>> {
        match state.get(&n) {
            Some(true) => return None,
            Some(false) => {
                let start = stack.iter().position(|s| *s == n).unwrap_or(0);
                let mut c = stack[start..].to_vec();
                c.push(n);
                return Some(c);
            }
            None => {}
        }
        state.insert(n, false);
        stack.push(n);
        for e in edges.iter().filter(|e| e.from == n) {
            if let Some(c) = visit(e.to, edges, state, stack) {
                return Some(c);
            }
        }
        stack.pop();
        state.insert(n, true);
        None
    }
    let mut state = HashMap::new();
    let nodes = (0..tickets)
        .map(PlanNode::Ticket)
        .chain((0..releases).map(PlanNode::Release));
    for n in nodes {
        if let Some(c) = visit(n, edges, &mut state, &mut Vec::new()) {
            return Some(c);
        }
    }
    None
}

fn quote(s: &str) -> String {
    toml::Value::String(s.to_owned()).to_string()
}

fn list(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|s| quote(s))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Serde name of a unit enum value (`tag_only`, `manual`).
fn enum_name<T: Serialize>(v: &T) -> String {
    toml::Value::try_from(v)
        .ok()
        .and_then(|x| x.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Plan {
    /// Turns the plan into a new chain of `project`. `key` hands out ticket keys in file order.
    /// Returns the view and the subtask texts of each ticket.
    pub fn instantiate(
        &self,
        project: ProjectId,
        now: Timestamp,
        mut key: impl FnMut() -> TicketKey,
    ) -> (ChainView, Vec<(TicketId, Vec<String>)>) {
        let chain = Chain {
            id: ChainId::new(),
            project_id: project,
            name: self.chain.name.clone(),
            goal: self.chain.goal.clone(),
            autostart: self.chain.autostart,
            review_mode: self.chain.review_mode,
        };
        let tickets: Vec<Ticket> = self
            .tickets
            .iter()
            .map(|t| Ticket {
                id: TicketId::new(),
                chain_id: chain.id,
                key: key(),
                title: t.title.clone(),
                prompt: t.prompt.clone(),
                acceptance: t.acceptance.clone(),
                status: TicketStatus::Backlog,
                status_reason: None,
                status_since: now,
                paused: t.paused,
                profile: t.profile.clone(),
                review_mode: t.review_mode,
                branch: None,
                needs_secrets: t.needs_secrets,
                tokens_used: 0,
            })
            .collect();
        let releases: Vec<Release> = self
            .releases
            .iter()
            .map(|r| Release {
                id: ReleaseId::new(),
                chain_id: chain.id,
                version: r.version.clone(),
                channel: r.channel,
                start_mode: r.start_mode,
                state: ReleaseState::Planned,
                branch: None,
                cut_at: None,
                released_at: None,
            })
            .collect();
        let node = |n: PlanNode| match n {
            PlanNode::Ticket(i) => NodeRef::Ticket(tickets[i].id),
            PlanNode::Release(i) => NodeRef::Release(releases[i].id),
        };
        let edges = self
            .edges
            .iter()
            .map(|e| Edge {
                chain_id: chain.id,
                from: node(e.from),
                to: node(e.to),
                kind: e.kind,
            })
            .collect();
        let subtasks = tickets
            .iter()
            .zip(&self.tickets)
            .map(|(t, p)| (t.id, p.subtasks.clone()))
            .collect();
        let view = ChainView {
            chain,
            tickets,
            releases,
            edges,
            release_tickets: vec![],
        };
        (view, subtasks)
    }
}

/// Writes a chain as a file that [`parse`] reads back into the same graph. Ticket keys and
/// release versions become the local ids. `subtasks` maps ticket ids to checklist texts.
pub fn export(view: &ChainView, subtasks: &HashMap<TicketId, Vec<String>>) -> String {
    let mut out = String::new();
    let c = &view.chain;
    let _ = writeln!(out, "[chain]\nname = {}", quote(&c.name));
    if !c.goal.is_empty() {
        let _ = writeln!(out, "goal = {}", quote(&c.goal));
    }
    if !c.autostart {
        let _ = writeln!(out, "autostart = false");
    }
    if let Some(m) = c.review_mode {
        let _ = writeln!(out, "review_mode = {}", quote(&enum_name(&m)));
    }
    let id_of = |n: NodeRef| match n {
        NodeRef::Ticket(t) => view
            .tickets
            .iter()
            .find(|x| x.id == t)
            .map(|x| x.key.to_string())
            .unwrap_or_default(),
        NodeRef::Release(r) => view
            .releases
            .iter()
            .find(|x| x.id == r)
            .map(|x| x.version.clone())
            .unwrap_or_default(),
    };
    let incoming = |n: NodeRef, kind: EdgeKind| -> Vec<String> {
        view.edges
            .iter()
            .filter(|e| e.to == n && e.kind == kind)
            .map(|e| id_of(e.from))
            .collect()
    };
    for node in view.topo_order() {
        match node {
            NodeRef::Ticket(id) => {
                let Some(t) = view.tickets.iter().find(|t| t.id == id) else {
                    continue;
                };
                let _ = writeln!(
                    out,
                    "\n[[ticket]]\nid = {}\ntitle = {}",
                    quote(&t.key.to_string()),
                    quote(&t.title)
                );
                if !t.prompt.is_empty() {
                    let _ = writeln!(out, "prompt = {}", quote(&t.prompt));
                }
                if !t.acceptance.is_empty() {
                    let _ = writeln!(out, "acceptance = {}", list(&t.acceptance));
                }
                if let Some(s) = subtasks.get(&t.id).filter(|s| !s.is_empty()) {
                    let _ = writeln!(out, "subtasks = {}", list(s));
                }
                let after = incoming(node, EdgeKind::Normal);
                if !after.is_empty() {
                    let _ = writeln!(out, "after = {}", list(&after));
                }
                let after_release = incoming(node, EdgeKind::AfterRelease);
                if !after_release.is_empty() {
                    let _ = writeln!(out, "after_release = {}", list(&after_release));
                }
                if let Some(p) = &t.profile {
                    let _ = writeln!(out, "profile = {}", quote(p));
                }
                if let Some(m) = t.review_mode {
                    let _ = writeln!(out, "review_mode = {}", quote(&enum_name(&m)));
                }
                if t.needs_secrets {
                    let _ = writeln!(out, "needs_secrets = true");
                }
                if t.paused {
                    let _ = writeln!(out, "paused = true");
                }
            }
            NodeRef::Release(id) => {
                let Some(r) = view.releases.iter().find(|r| r.id == id) else {
                    continue;
                };
                let _ = writeln!(
                    out,
                    "\n[[release]]\nid = {}\nchannel = {}\nstart = {}",
                    quote(&r.version),
                    quote(&enum_name(&r.channel)),
                    quote(&enum_name(&r.start_mode))
                );
                let after = incoming(node, EdgeKind::Normal);
                if !after.is_empty() {
                    let _ = writeln!(out, "after = {}", list(&after));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
