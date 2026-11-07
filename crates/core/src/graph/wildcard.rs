//! Wildcard cards: work nobody planned joins the chain as a proposal and touches the graph only
//! once a person approves it. Until then it has no edges, so it never starts and changes no
//! other node's readiness; its dependencies wait on the card itself.

use super::*;
use crate::wire::wire_name;

impl ChainView {
    /// Adds a wildcard card in Proposed. Its pending dependencies must be nodes of this chain.
    pub fn propose(&mut self, card: Ticket) -> Result<(), GraphError> {
        let Some(mark) = card
            .wildcard
            .as_ref()
            .filter(|_| card.status == TicketStatus::Proposed)
        else {
            return Err(GraphError::NotProposed(card.key.to_string()));
        };
        let nodes = self.nodes();
        if let Some(n) = mark
            .after
            .iter()
            .chain(&mark.before)
            .find(|n| !nodes.contains(n))
        {
            return Err(GraphError::UnknownNode(*n));
        }
        self.tickets.push(card);
        Ok(())
    }

    /// Turns an approved card's pending dependencies into edges: `after` nodes come before the
    /// card, `before` nodes after it. Work that has started cannot be put after the card, and
    /// the graph must stay valid (no cycle, no edge to a card still proposed). On error the
    /// edges are unchanged.
    pub(super) fn link_proposed(
        &mut self,
        card: TicketId,
        after: &[NodeRef],
        before: &[NodeRef],
    ) -> Result<(), GraphError> {
        for &n in before {
            if let Some(state) = self.started(n) {
                return Err(GraphError::BeforeStarted {
                    node: self.label(n),
                    state,
                });
            }
        }
        let chain_id = self.chain.id;
        let card = NodeRef::Ticket(card);
        let edge = |from, to| Edge {
            chain_id,
            from,
            to,
            kind: EdgeKind::Normal,
        };
        let kept = self.edges.len();
        self.edges.extend(after.iter().map(|&n| edge(n, card)));
        self.edges.extend(before.iter().map(|&n| edge(card, n)));
        if let Err(e) = self.validate() {
            self.edges.truncate(kept);
            return Err(e);
        }
        Ok(())
    }

    /// The state of a node that has already started (as its wire name), or `None` if it has
    /// not: a ticket still in Backlog or Ready, a milestone still planned. A card still proposed
    /// has not started either; [`Self::validate`] refuses edges to it.
    pub(super) fn started(&self, node: NodeRef) -> Option<String> {
        match node {
            NodeRef::Ticket(id) => self
                .ticket(id)
                .map(|t| t.status)
                .filter(|s| {
                    !matches!(
                        s,
                        TicketStatus::Proposed | TicketStatus::Backlog | TicketStatus::Ready
                    )
                })
                .and_then(|s| wire_name(&s)),
            NodeRef::Release(id) => self
                .release(id)
                .map(|r| r.state)
                .filter(|s| *s != ReleaseState::Planned)
                .and_then(|s| wire_name(&s)),
        }
    }

    /// The first ticket in Proposed among `nodes`, as its label.
    pub(super) fn first_proposed(
        &self,
        nodes: impl IntoIterator<Item = NodeRef>,
    ) -> Option<String> {
        nodes
            .into_iter()
            .find(|n| match n {
                NodeRef::Ticket(id) => self
                    .ticket(*id)
                    .is_some_and(|t| t.status == TicketStatus::Proposed),
                NodeRef::Release(_) => false,
            })
            .map(|n| self.label(n))
    }
}
