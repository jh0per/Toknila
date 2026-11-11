//! Reading a chain file: TOML into a validated [`Plan`] (unique ids, known references, no
//! self-loops, repeats or cycles), each problem reported with its line.

use std::collections::HashMap;
use std::ops::Range;

use toml::Spanned;

use super::raw::RawFile;
use super::{Plan, PlanChain, PlanDesign, PlanEdge, PlanError, PlanNode, PlanRelease, PlanTicket};
use crate::dag::{self, EdgeCheck, EdgeProblem};
use crate::{Chain, EdgeKind};

/// Parses and validates a chain file.
pub fn parse(text: &str) -> Result<Plan, PlanError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| PlanError {
        line: e.span().map_or(1, |s| line_of(text, s.start)),
        message: e.message().to_owned(),
    })?;
    let file = Source { text, raw: &raw };
    let ids = file.ids()?;
    file.check_done_subtasks()?;
    file.check_design_regions()?;
    let edges = file.edges(&ids)?;
    file.check_edges(&edges)?;
    Ok(build(raw, edges))
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// The file text with its raw parse, for errors that name a line.
struct Source<'a> {
    text: &'a str,
    raw: &'a RawFile,
}

type Ids = HashMap<String, PlanNode>;

impl Source<'_> {
    fn error(&self, span: Range<usize>, message: String) -> PlanError {
        PlanError {
            line: line_of(self.text, span.start),
            message,
        }
    }

    /// Local ids of tickets, then releases; each must be unique and tickets' non-empty.
    fn ids(&self) -> Result<Ids, PlanError> {
        let mut ids = Ids::new();
        let tickets = self.raw.ticket.iter().enumerate();
        let tickets = tickets.map(|(i, t)| (&t.id, PlanNode::Ticket(i)));
        let releases = self.raw.release.iter().enumerate();
        let releases = releases.map(|(i, r)| (&r.id, PlanNode::Release(i)));
        for (id, node) in tickets.chain(releases) {
            if matches!(node, PlanNode::Ticket(_)) && id.get_ref().trim().is_empty() {
                return Err(self.error(id.span(), "ticket id is empty".into()));
            }
            if ids.insert(id.get_ref().clone(), node).is_some() {
                let message = format!("duplicate id {:?}", id.get_ref());
                return Err(self.error(id.span(), message));
            }
        }
        Ok(ids)
    }

    /// `done_subtasks` positions must point at existing subtasks.
    fn check_done_subtasks(&self) -> Result<(), PlanError> {
        for t in &self.raw.ticket {
            let len = t.subtasks.len();
            let bad = t
                .done_subtasks
                .iter()
                .find(|n| **n == 0 || **n as usize > len);
            if let Some(bad) = bad {
                let message = format!("done_subtasks {bad} is not one of {len} subtasks");
                return Err(self.error(t.id.span(), message));
            }
        }
        Ok(())
    }

    /// Every region of a design reference lies inside the image.
    fn check_design_regions(&self) -> Result<(), PlanError> {
        for t in &self.raw.ticket {
            for d in &t.designs {
                if let Some(Err(why)) = d.region.as_ref().map(crate::design::Region::check) {
                    let message = format!("design {:?}: {why}", d.design);
                    return Err(self.error(t.id.span(), message));
                }
            }
        }
        Ok(())
    }

    fn node(&self, ids: &Ids, s: &Spanned<String>) -> Result<PlanNode, PlanError> {
        ids.get(s.get_ref())
            .copied()
            .ok_or_else(|| self.error(s.span(), format!("unknown id {:?}", s.get_ref())))
    }

    /// Edges in file order: tickets' `after` and `after_release`, then releases' `after` and
    /// `before`.
    fn edges(&self, ids: &Ids) -> Result<Vec<PlanEdge>, PlanError> {
        let edge = |from, to, kind| PlanEdge { from, to, kind };
        let mut edges = Vec::new();
        for (i, t) in self.raw.ticket.iter().enumerate() {
            let to = PlanNode::Ticket(i);
            for a in &t.after {
                edges.push(edge(self.node(ids, a)?, to, EdgeKind::Normal));
            }
            for a in &t.after_release {
                let from = self.node(ids, a)?;
                if !matches!(from, PlanNode::Release(_)) {
                    let message = format!(
                        "after_release needs a release, {:?} is a ticket",
                        a.get_ref()
                    );
                    return Err(self.error(a.span(), message));
                }
                edges.push(edge(from, to, EdgeKind::AfterRelease));
            }
        }
        for (i, r) in self.raw.release.iter().enumerate() {
            let node = PlanNode::Release(i);
            for a in &r.after {
                edges.push(edge(self.node(ids, a)?, node, EdgeKind::Normal));
            }
            for b in &r.before {
                edges.push(edge(node, self.node(ids, b)?, EdgeKind::Normal));
            }
        }
        Ok(edges)
    }

    fn name_of(&self, n: PlanNode) -> String {
        self.id_of(n).get_ref().clone()
    }

    fn line_of_node(&self, n: PlanNode) -> usize {
        line_of(self.text, self.id_of(n).span().start)
    }

    fn id_of(&self, n: PlanNode) -> &Spanned<String> {
        match n {
            PlanNode::Ticket(i) => &self.raw.ticket[i].id,
            PlanNode::Release(i) => &self.raw.release[i].id,
        }
    }

    /// No self-loops, no repeated pairs, no cycles.
    fn check_edges(&self, edges: &[PlanEdge]) -> Result<(), PlanError> {
        let mut check = EdgeCheck::default();
        for e in edges {
            let message = match check.check(e.from, e.to) {
                Ok(()) => continue,
                Err(EdgeProblem::SelfLoop(n)) => format!("{} depends on itself", self.name_of(n)),
                Err(EdgeProblem::Duplicate { from, to }) => format!(
                    "{} -> {} is listed twice",
                    self.name_of(from),
                    self.name_of(to)
                ),
            };
            return Err(PlanError {
                line: self.line_of_node(e.to),
                message,
            });
        }
        let tickets = (0..self.raw.ticket.len()).map(PlanNode::Ticket);
        let releases = (0..self.raw.release.len()).map(PlanNode::Release);
        let successors = |n: PlanNode| {
            let out = edges.iter().filter(|e| e.from == n);
            out.map(|e| e.to).collect()
        };
        let Some(cycle) = dag::find_cycle(tickets.chain(releases), successors) else {
            return Ok(());
        };
        let path: Vec<String> = cycle.iter().map(|n| self.name_of(*n)).collect();
        Err(PlanError {
            line: self.line_of_node(cycle[0]),
            message: format!("dependency cycle: {}", path.join(" -> ")),
        })
    }
}

fn build(raw: RawFile, edges: Vec<PlanEdge>) -> Plan {
    let chain = PlanChain {
        name: raw.chain.name,
        goal: raw.chain.goal,
        autostart: raw.chain.autostart.unwrap_or(Chain::DEFAULT_AUTOSTART),
        review_mode: raw.chain.review_mode,
    };
    let tickets = raw.ticket.into_iter().map(|t| PlanTicket {
        title: t.title.unwrap_or_else(|| t.id.get_ref().clone()),
        local_id: t.id.into_inner(),
        prompt: t.prompt,
        acceptance: t.acceptance,
        subtasks: t.subtasks,
        profile: t.profile,
        review_mode: t.review_mode,
        needs_secrets: t.needs_secrets,
        design_first: t.design_first,
        designs: t
            .designs
            .into_iter()
            .map(|d| PlanDesign {
                design: d.design,
                region: d.region,
            })
            .collect(),
        paused: t.paused,
        done: t.done,
        done_subtasks: t.done_subtasks,
    });
    let releases = raw.release.into_iter().map(|r| PlanRelease {
        version: r.version.unwrap_or_else(|| r.id.get_ref().clone()),
        local_id: r.id.into_inner(),
        channel: r.channel.unwrap_or_default(),
        start_mode: r.start.unwrap_or_default(),
    });
    Plan {
        chain,
        tickets: tickets.collect(),
        releases: releases.collect(),
        edges,
    }
}
