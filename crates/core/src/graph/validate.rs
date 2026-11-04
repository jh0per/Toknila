//! Graph shape checks: known nodes, no self-loops, repeats or cycles.

use super::*;
use crate::dag::{self, EdgeCheck, EdgeProblem};

impl ChainView {
    /// Checks edges: known nodes, no self-loops or duplicates, `after_release` only from a
    /// milestone, and no cycles (the error names the cycle).
    pub fn validate(&self) -> Result<(), GraphError> {
        let nodes = self.nodes();
        let mut check = EdgeCheck::default();
        for e in &self.edges {
            for n in [e.from, e.to] {
                if !nodes.contains(&n) {
                    return Err(GraphError::UnknownNode(n));
                }
            }
            check.check(e.from, e.to).map_err(|p| match p {
                EdgeProblem::SelfLoop(n) => GraphError::SelfLoop(n),
                EdgeProblem::Duplicate { from, to } => GraphError::DuplicateEdge { from, to },
            })?;
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

    fn find_cycle(&self) -> Option<Vec<NodeRef>> {
        let succ = self.successors();
        // ULIDs minted in the same millisecond are unordered; walk in key order so the
        // reported cycle is the same on every run.
        let mut roots: Vec<NodeRef> = self.nodes().into_iter().collect();
        roots.sort_by_key(|n| self.sort_key(*n));
        dag::find_cycle(roots, |n| succ.get(&n).cloned().unwrap_or_default())
    }
}
