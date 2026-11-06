//! The inbox of a project: load what core's rules need and let core compute the list.
//! Nothing here decides what waits on a person; see `toknila_core::inbox`.

use std::collections::BTreeSet;

use toknila_core::inbox::{InboxData, InboxItem, ThreadFacts, compute};
use toknila_core::{ProjectId, Ticket, TicketKey};

use crate::{AppResult, Reader};

/// Every ticket of a project, chain by chain.
pub(crate) fn project_tickets<R: Reader + ?Sized>(
    r: &R,
    project: ProjectId,
) -> AppResult<Vec<Ticket>> {
    let mut out = Vec::new();
    for chain in r.chains(Some(project))? {
        out.extend(r.tickets(Some(chain.id))?);
    }
    Ok(out)
}

/// What waits on a person in `project`, newest first.
pub fn load<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<Vec<InboxItem>> {
    let tickets = project_tickets(r, project)?;
    let mut data = InboxData::default();
    // The earlier cards of the cards done out of order hold the subtasks that may be left over.
    let behind: BTreeSet<&TicketKey> = tickets
        .iter()
        .filter_map(|t| t.done_out_of_order.as_ref())
        .flat_map(|m| &m.behind)
        .collect();
    for card in tickets.iter().filter(|t| behind.contains(&t.key)) {
        data.subtasks.extend(r.subtasks(card.id)?);
    }
    for t in &tickets {
        for thread in r.threads(t.id)? {
            data.threads.push(ThreadFacts {
                ticket: t.key.clone(),
                versions: r.thread_versions(thread.id)?,
                messages: r.thread_messages(thread.id)?,
                thread,
            });
        }
    }
    data.tickets = tickets;
    Ok(compute(&data))
}
