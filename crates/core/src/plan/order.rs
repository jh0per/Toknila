//! The out-of-order rule: a card that is done while a card before it is not.

use super::{Plan, PlanNode};

/// For each node, the nodes before it (directly or through other nodes) that are not done, when
/// the node itself is done; empty for a node that is not done or has nothing open before it.
/// `after[i]` lists what node `i` waits for. Indices come back in ascending order.
pub fn out_of_order(done: &[bool], after: &[Vec<usize>]) -> Vec<Vec<usize>> {
    (0..done.len())
        .map(|i| {
            if !done[i] {
                return Vec::new();
            }
            let mut seen = vec![false; done.len()];
            let mut stack: Vec<usize> = after[i].clone();
            let mut open = Vec::new();
            while let Some(n) = stack.pop() {
                if std::mem::replace(&mut seen[n], true) {
                    continue;
                }
                if !done[n] {
                    open.push(n);
                }
                stack.extend(&after[n]);
            }
            open.sort_unstable();
            open
        })
        .collect()
}

impl Plan {
    /// For each ticket (by index) that is marked done, the tickets before it that are not. A
    /// milestone between two tickets passes the order on; it is never reported itself.
    pub fn out_of_order(&self) -> Vec<Vec<usize>> {
        let tickets = self.tickets.len();
        let index = |n: PlanNode| match n {
            PlanNode::Ticket(i) => i,
            PlanNode::Release(i) => tickets + i,
        };
        let mut done: Vec<bool> = self.tickets.iter().map(|t| t.done).collect();
        done.resize(tickets + self.releases.len(), true);
        let mut after = vec![Vec::new(); done.len()];
        for e in &self.edges {
            after[index(e.to)].push(index(e.from));
        }
        let mut out = out_of_order(&done, &after);
        out.truncate(tickets);
        out
    }
}
