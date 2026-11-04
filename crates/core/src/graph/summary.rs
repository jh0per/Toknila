//! Computed chain status, progress, where it stopped and the next milestone.

use super::*;

impl ChainView {
    /// Status, progress, where it stopped and the next milestone, computed from tickets.
    pub fn summary(&self, now: Timestamp) -> ChainSummary {
        use TicketStatus as S;
        let planned = || self.tickets.iter().filter(|t| t.in_plan());
        let total = planned().count();
        let count = |f: &dyn Fn(S) -> bool| planned().filter(|t| f(t.status)).count();
        let done = count(&|s| s.is_done());
        let released = count(&|s| s == S::Released);
        let any = |f: &dyn Fn(S) -> bool| self.tickets.iter().any(|t| f(t.status));
        let proposed = any(&|s| s == S::Proposed);

        let order = self.topo_order();
        let in_order = order.iter().filter_map(|n| match n {
            NodeRef::Ticket(id) => self.ticket(*id),
            NodeRef::Release(_) => None,
        });
        let stopped_at = in_order
            .clone()
            .find(|t| matches!(t.status, S::Waiting | S::Blocked))
            .or_else(|| in_order.clone().find(|t| t.status == S::Review))
            .or_else(|| in_order.clone().find(|t| t.status == S::Proposed))
            .map(|t| StoppedAt {
                ticket: t.key.clone(),
                status: t.status,
                reason: t.status_reason.clone(),
                since: t.status_since,
                waiting_ms: now.millis_since(t.status_since),
            });
        let next_release = order
            .iter()
            .filter_map(|n| match n {
                NodeRef::Release(id) => self.release(*id),
                NodeRef::Ticket(_) => None,
            })
            .find(|r| r.state != ReleaseState::Released)
            .map(|r| NextRelease {
                release: r.id,
                version: r.version.clone(),
                channel: r.channel,
                state: r.state,
            });

        let all_released = self
            .releases
            .iter()
            .all(|r| r.state == ReleaseState::Released);
        let status = if total == 0 && !proposed {
            ChainStatus::Empty
        } else if total > 0 && done == total && all_released && !proposed {
            ChainStatus::Done
        } else if any(&|s| s == S::Blocked) {
            ChainStatus::Blocked
        } else if any(&|s| matches!(s, S::Waiting | S::Review | S::Proposed)) {
            ChainStatus::NeedsHuman
        } else if any(&|s| s == S::Running) {
            ChainStatus::Running
        } else if planned().all(|t| t.status == S::Backlog && !t.paused)
            && self
                .releases
                .iter()
                .all(|r| r.state == ReleaseState::Planned)
        {
            ChainStatus::NotStarted
        } else {
            ChainStatus::Idle
        };
        ChainSummary {
            status,
            total,
            done,
            released,
            stopped_at,
            next_release,
        }
    }
}
