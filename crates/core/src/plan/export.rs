//! Writing a chain as a file that [`super::parse`] reads back into the same graph.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::graph::ChainView;
use crate::wire::wire_name;
use crate::{EdgeKind, NodeRef, Release, Ticket, TicketId};

/// Writes a chain as a file that [`super::parse`] reads back into the same graph. Ticket keys
/// and release versions become the local ids. `subtasks` maps ticket ids to checklist texts.
/// Wildcard cards that are not in the plan (proposed or rejected) are left out; no edge touches
/// them, so the file stays whole.
pub fn export(view: &ChainView, subtasks: &HashMap<TicketId, Vec<String>>) -> String {
    let mut out = Lines::default();
    chain_section(view, &mut out);
    for node in view.topo_order() {
        match node {
            NodeRef::Ticket(id) => {
                if let Some(t) = view.tickets.iter().find(|t| t.id == id && t.in_plan()) {
                    ticket_section(view, t, subtasks.get(&t.id), &mut out);
                }
            }
            NodeRef::Release(id) => {
                if let Some(r) = view.releases.iter().find(|r| r.id == id) {
                    release_section(view, r, &mut out);
                }
            }
        }
    }
    out.0
}

/// TOML text being written, one `key = value` per line.
#[derive(Default)]
struct Lines(String);

impl Lines {
    fn raw(&mut self, line: &str) {
        let _ = writeln!(self.0, "{line}");
    }

    fn str(&mut self, key: &str, value: &str) {
        let _ = writeln!(self.0, "{key} = {}", quote(value));
    }

    fn list(&mut self, key: &str, items: &[String]) {
        if !items.is_empty() {
            let items: Vec<String> = items.iter().map(|s| quote(s)).collect();
            let _ = writeln!(self.0, "{key} = [{}]", items.join(", "));
        }
    }
}

fn quote(s: &str) -> String {
    toml::Value::String(s.to_owned()).to_string()
}

fn chain_section(view: &ChainView, out: &mut Lines) {
    let c = &view.chain;
    out.raw("[chain]");
    out.str("name", &c.name);
    if !c.goal.is_empty() {
        out.str("goal", &c.goal);
    }
    if !c.autostart {
        out.raw("autostart = false");
    }
    if let Some(m) = c.review_mode {
        out.str("review_mode", &wire_name(&m).unwrap_or_default());
    }
}

fn ticket_section(view: &ChainView, t: &Ticket, subtasks: Option<&Vec<String>>, out: &mut Lines) {
    let node = NodeRef::Ticket(t.id);
    out.raw("\n[[ticket]]");
    out.str("id", &t.key.to_string());
    out.str("title", &t.title);
    if !t.prompt.is_empty() {
        out.str("prompt", &t.prompt);
    }
    out.list("acceptance", &t.acceptance);
    out.list("subtasks", subtasks.map_or(&[], Vec::as_slice));
    out.list("after", &incoming(view, node, EdgeKind::Normal));
    out.list(
        "after_release",
        &incoming(view, node, EdgeKind::AfterRelease),
    );
    if let Some(p) = &t.profile {
        out.str("profile", p);
    }
    if let Some(m) = t.review_mode {
        out.str("review_mode", &wire_name(&m).unwrap_or_default());
    }
    if t.needs_secrets {
        out.raw("needs_secrets = true");
    }
    if t.design_first {
        out.raw("design_first = true");
    }
    if t.paused {
        out.raw("paused = true");
    }
}

fn release_section(view: &ChainView, r: &Release, out: &mut Lines) {
    out.raw("\n[[release]]");
    out.str("id", &r.version);
    out.str("channel", &wire_name(&r.channel).unwrap_or_default());
    out.str("start", &wire_name(&r.start_mode).unwrap_or_default());
    out.list(
        "after",
        &incoming(view, NodeRef::Release(r.id), EdgeKind::Normal),
    );
}

/// Local ids of the nodes `node` waits for through edges of `kind`.
fn incoming(view: &ChainView, node: NodeRef, kind: EdgeKind) -> Vec<String> {
    view.edges
        .iter()
        .filter(|e| e.to == node && e.kind == kind)
        .map(|e| local_id(view, e.from))
        .collect()
}

/// A node's id in an exported file: ticket key or release version.
fn local_id(view: &ChainView, n: NodeRef) -> String {
    match n {
        NodeRef::Ticket(t) => view
            .tickets
            .iter()
            .find(|x| x.id == t)
            .map(|x| x.key.to_string()),
        NodeRef::Release(r) => view
            .releases
            .iter()
            .find(|x| x.id == r)
            .map(|x| x.version.clone()),
    }
    .unwrap_or_default()
}
