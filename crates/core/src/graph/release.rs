//! Release milestones: cutting a milestone and completing it.

use super::*;

impl ChainView {
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

    /// Records the git tag a release run made for a milestone.
    pub fn tag_release(&mut self, release: ReleaseId, tag: String) -> Result<(), GraphError> {
        let r = self
            .releases
            .iter_mut()
            .find(|r| r.id == release)
            .ok_or(GraphError::UnknownRelease(release))?;
        r.tag = Some(tag);
        Ok(())
    }

    /// The milestone a release of the whole chain goes through: the last one nothing waits for
    /// that is not released yet.
    pub fn end_release(&self) -> Option<&Release> {
        let successors = self.successors();
        self.topo_order().into_iter().rev().find_map(|n| match n {
            NodeRef::Release(id) if successors.get(&n).is_none_or(Vec::is_empty) => self
                .release(id)
                .filter(|r| r.state != ReleaseState::Released),
            _ => None,
        })
    }

    /// The cards no other card waits for: a new milestone for the whole chain waits for them.
    pub fn leaf_tickets(&self) -> Vec<TicketId> {
        let successors = self.successors();
        self.tickets
            .iter()
            .filter(|t| t.in_plan())
            .filter(|t| {
                successors
                    .get(&NodeRef::Ticket(t.id))
                    .is_none_or(Vec::is_empty)
            })
            .map(|t| t.id)
            .collect()
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
}
