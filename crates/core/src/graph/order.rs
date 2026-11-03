//! Dependency order and readiness: topological order, satisfied edges, ancestors.

use super::*;

impl ChainView {
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
    pub(super) fn edge_satisfied(&self, edge: &Edge) -> bool {
        match edge.from {
            NodeRef::Ticket(id) => self.ticket(id).is_some_and(|t| t.status.is_done()),
            NodeRef::Release(id) => self.release(id).is_some_and(|r| match edge.kind {
                EdgeKind::Normal => matches!(r.state, ReleaseState::Cut | ReleaseState::Released),
                EdgeKind::AfterRelease => r.state == ReleaseState::Released,
            }),
        }
    }

    pub(super) fn unsatisfied_deps(&self, node: NodeRef) -> Vec<NodeRef> {
        self.edges
            .iter()
            .filter(|e| e.to == node && !self.edge_satisfied(e))
            .map(|e| e.from)
            .collect()
    }

    pub(super) fn ancestors(&self, node: NodeRef) -> BTreeSet<NodeRef> {
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
}
