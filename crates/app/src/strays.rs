//! Ships and branches no live card owns. The rule is core's (`toknila_core::strays`); this
//! module reads what the project's live cards hold, the host looks at the repository.

use toknila_core::strays::Owned;
use toknila_core::{Project, ShipSpec, TicketStatus};

use crate::{AppResult, Reader};

/// What the live (not cancelled) cards of `project` hold: the branch each has or would have, and
/// the worktree it records. Cards of deleted chains are gone, so they hold nothing.
pub fn owned<R: Reader + ?Sized>(r: &R, project: &Project) -> AppResult<Owned> {
    let mut owned = Owned::default();
    for chain in r.chains(Some(project.id))? {
        for t in r.tickets(Some(chain.id))? {
            if matches!(t.status, TicketStatus::Cancelled | TicketStatus::Merged) {
                // Done with: its branch is a leftover, named by the card it belonged to.
                let key = t.key.to_string();
                owned
                    .retired
                    .extend(t.branch.clone().map(|b| (b, key.clone())));
                if let Some(ship) = r.ship(t.id)? {
                    owned.retired.insert(ship.branch, key.clone());
                }
                owned.retired.extend(
                    ShipSpec::for_ticket(project, &chain, &t).map(|s| (s.branch, key.clone())),
                );
                continue;
            }
            owned.branches.extend(t.branch.clone());
            owned
                .branches
                .extend(ShipSpec::for_ticket(project, &chain, &t).map(|s| s.branch));
            if let Some(ship) = r.ship(t.id)? {
                owned.branches.insert(ship.branch);
                owned.worktrees.insert(ship.path);
            }
        }
    }
    Ok(owned)
}
