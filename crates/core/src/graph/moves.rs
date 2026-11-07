//! Moving a card inside the chain. A chain has no position field: the order of cards is the
//! dependency edges, so "move" means rewiring them. The rewiring is a pure plan the caller
//! applies edge by edge.

use super::*;

/// Where a card goes relative to another one.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// The card becomes a dependency of the anchor: it runs just before it.
    Before,
    /// The card depends on the anchor: it runs just after it.
    After,
}

/// The edges a move removes and adds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MovePlan {
    pub remove: Vec<Edge>,
    pub add: Vec<Edge>,
}

impl ChainView {
    /// Can the card be moved, or have its dependencies changed? Only a card that has not started.
    pub fn can_move(&self, card: TicketId) -> bool {
        self.ticket(card).is_some_and(|t| t.status.can_move())
    }

    /// The cards a new card is appended after: the tickets nothing waits for, ignoring
    /// cancelled ones and cards still proposed.
    pub fn tail_tickets(&self) -> Vec<TicketId> {
        let open =
            |t: &&Ticket| !matches!(t.status, TicketStatus::Proposed | TicketStatus::Cancelled);
        self.tickets
            .iter()
            .filter(open)
            .filter(|t| !self.edges.iter().any(|e| e.from == NodeRef::Ticket(t.id)))
            .map(|t| t.id)
            .collect()
    }

    /// The edge changes that put `card` directly before or after `anchor`. The card leaves its
    /// place first and its neighbours are joined, so the rest of the order stays as it was; then
    /// it takes the anchor's place in series. Neither the card nor the work it ends up before
    /// may have started.
    pub fn plan_move(
        &self,
        card: TicketId,
        anchor: TicketId,
        place: Place,
    ) -> Result<MovePlan, GraphError> {
        let (c, a) = (NodeRef::Ticket(card), NodeRef::Ticket(anchor));
        for id in [card, anchor] {
            self.ticket(id).ok_or(GraphError::UnknownTicket(id))?;
        }
        if card == anchor {
            return Err(GraphError::SelfLoop(c));
        }
        let refuse = |node: NodeRef, state: Option<String>| GraphError::BeforeStarted {
            node: self.label(node),
            state: state.unwrap_or_else(|| "proposed".to_owned()),
        };
        if !self.can_move(card) {
            return Err(refuse(c, self.started(c)));
        }
        let mut edges = self.edges.clone();
        let edge = |from, to, kind| Edge {
            chain_id: self.chain.id,
            from,
            to,
            kind,
        };
        let (preds, succs): (Vec<Edge>, Vec<Edge>) = edges
            .iter()
            .filter(|e| e.from == c || e.to == c)
            .partition(|e| e.to == c);
        edges.retain(|e| e.from != c && e.to != c);
        for p in &preds {
            for s in &succs {
                edges.push(edge(p.from, s.to, p.kind));
            }
        }
        // The edges the card takes over: the anchor's outgoing ones to go after it, its incoming
        // ones to go before it. Whatever ends up waiting for the card must not have started.
        let (moved, waits): (Vec<Edge>, Vec<NodeRef>) = match place {
            Place::After => {
                let moved: Vec<Edge> = edges.iter().filter(|e| e.from == a).copied().collect();
                let waits = moved.iter().map(|e| e.to).collect();
                (moved, waits)
            }
            Place::Before => (
                edges.iter().filter(|e| e.to == a).copied().collect(),
                vec![a],
            ),
        };
        if let Some(n) = waits.iter().find(|n| self.started(**n).is_some()) {
            return Err(refuse(*n, self.started(*n)));
        }
        edges.retain(|e| !moved.contains(e));
        match place {
            Place::After => {
                edges.push(edge(a, c, EdgeKind::Normal));
                edges.extend(moved.iter().map(|e| edge(c, e.to, EdgeKind::Normal)));
            }
            Place::Before => {
                edges.extend(moved.iter().map(|e| edge(e.from, c, e.kind)));
                edges.push(edge(c, a, EdgeKind::Normal));
            }
        }
        let mut seen = BTreeSet::new();
        edges.retain(|e| seen.insert((e.from, e.to)));
        let remove = self
            .edges
            .iter()
            .filter(|e| !edges.contains(e))
            .copied()
            .collect();
        let add = edges
            .iter()
            .filter(|e| !self.edges.contains(e))
            .copied()
            .collect();
        Ok(MovePlan { remove, add })
    }
}
