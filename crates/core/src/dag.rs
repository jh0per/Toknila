//! Directed-graph checks shared by the chain graph ([`crate::graph`]) and chain files
//! ([`crate::plan`]), so both reject the same shapes in the same way.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// What is wrong with one edge on its own or next to the edges seen before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeProblem<N> {
    /// The edge starts and ends at the same node.
    SelfLoop(N),
    /// The same `from -> to` pair was seen before.
    Duplicate { from: N, to: N },
}

/// Checks edges one at a time for self-loops and repeats.
#[derive(Debug)]
pub struct EdgeCheck<N> {
    seen: HashSet<(N, N)>,
}

impl<N: Copy + Eq + Hash> Default for EdgeCheck<N> {
    fn default() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }
}

impl<N: Copy + Eq + Hash> EdgeCheck<N> {
    /// Accepts `from -> to` or says why not; a refused edge is not remembered.
    pub fn check(&mut self, from: N, to: N) -> Result<(), EdgeProblem<N>> {
        if from == to {
            return Err(EdgeProblem::SelfLoop(from));
        }
        if !self.seen.insert((from, to)) {
            return Err(EdgeProblem::Duplicate { from, to });
        }
        Ok(())
    }
}

/// The first cycle found by a depth-first walk that starts from each of `roots` in order and
/// follows `successors` in their given order. The path starts and ends at the same node.
/// `None` when the graph is acyclic.
pub fn find_cycle<N, F>(roots: impl IntoIterator<Item = N>, successors: F) -> Option<Vec<N>>
where
    N: Copy + Eq + Hash,
    F: Fn(N) -> Vec<N>,
{
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<N: Copy + Eq + Hash>(
        n: N,
        successors: &dyn Fn(N) -> Vec<N>,
        marks: &mut HashMap<N, Mark>,
        stack: &mut Vec<N>,
    ) -> Option<Vec<N>> {
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
        for next in successors(n) {
            if let Some(c) = visit(next, successors, marks, stack) {
                return Some(c);
            }
        }
        stack.pop();
        marks.insert(n, Mark::Done);
        None
    }
    let mut marks = HashMap::new();
    for root in roots {
        if let Some(c) = visit(root, &successors, &mut marks, &mut Vec::new()) {
            return Some(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn succ(edges: &[(u8, u8)]) -> impl Fn(u8) -> Vec<u8> + '_ {
        move |n| {
            edges
                .iter()
                .filter(|(f, _)| *f == n)
                .map(|(_, t)| *t)
                .collect()
        }
    }

    #[test]
    fn acyclic_graph_has_no_cycle() {
        let edges = [(1, 2), (2, 3), (1, 3)];
        assert_eq!(find_cycle([1, 2, 3], succ(&edges)), None);
    }

    #[test]
    fn cycle_is_reported_as_a_closed_path() {
        let edges = [(1, 2), (2, 3), (3, 2)];
        assert_eq!(find_cycle([1, 2, 3], succ(&edges)), Some(vec![2, 3, 2]));
    }

    #[test]
    fn root_order_decides_which_cycle_is_found_first() {
        let edges = [(1, 2), (2, 1), (3, 4), (4, 3)];
        assert_eq!(find_cycle([3, 1], succ(&edges)), Some(vec![3, 4, 3]));
    }

    #[test]
    fn self_loops_and_repeats_are_refused() {
        let mut check = EdgeCheck::default();
        assert_eq!(check.check(1, 2), Ok(()));
        assert_eq!(check.check(3, 3), Err(EdgeProblem::SelfLoop(3)));
        assert_eq!(
            check.check(1, 2),
            Err(EdgeProblem::Duplicate { from: 1, to: 2 })
        );
        assert_eq!(check.check(2, 1), Ok(()));
    }
}
