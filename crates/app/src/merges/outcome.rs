//! How a merge's outcome on the host reaches its card: `MergeDone`, a re-check, or a block with
//! a typed reason.

use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, MergeSettings, PullRequest, ShipSpec, TicketId};

use super::{MergeError, MergeOutcome};
use crate::chain::tell_ticket;
use crate::{AppResult, UnitOfWork, sync};

/// The host answered: the ticket is told through the machine.
pub(crate) fn finished<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    result: Result<MergeOutcome, MergeError>,
) -> AppResult<()> {
    let event = match result {
        Ok(MergeOutcome::Merged { .. } | MergeOutcome::UpToDate) => TicketEvent::MergeDone,
        Ok(MergeOutcome::PrMerged { pr }) => {
            record_pr(uow, ticket, pr)?;
            TicketEvent::MergeDone
        }
        Ok(MergeOutcome::PrOpen { pr }) => return record_pr(uow, ticket, pr),
        Ok(MergeOutcome::MainMoved { commits }) => TicketEvent::MainMoved {
            text: match commits {
                Some(1) => "main moved since the check: 1 commit".to_owned(),
                Some(n) => format!("main moved since the check: {n} commits"),
                None => "main moved since the check".to_owned(),
            },
        },
        Ok(MergeOutcome::CiFailed { pr, failed }) => {
            let url = pr.url.clone();
            record_pr(uow, ticket, pr)?;
            block(BlockedReason::CiFailed {
                url,
                checks: failed,
            })
        }
        Ok(MergeOutcome::PrClosed { url }) => block(BlockedReason::PrClosed { url }),
        Ok(MergeOutcome::PrConflict { url }) => {
            let text = format!(
                "GitHub reports a merge conflict on {url}. Merge the base branch into the ship, resolve it, run the gates, commit, then finish again."
            );
            conflict(uow, ticket, text)?
        }
        Ok(MergeOutcome::BaseConflict { detail }) => conflict(uow, ticket, detail)?,
        Ok(MergeOutcome::Conflict { files }) => {
            let base = base_of(uow, ticket)?.unwrap_or_else(|| "the base branch".to_owned());
            let text = sync::conflict_text(&base, &files, &[]);
            conflict(uow, ticket, text)?
        }
        Ok(MergeOutcome::ShipUncommitted { files }) => {
            block(BlockedReason::ShipUncommitted { files })
        }
        Ok(MergeOutcome::ShipEmpty) => block(BlockedReason::ShipEmpty),
        Ok(MergeOutcome::BaseCheckoutDirty { path }) => {
            block(BlockedReason::BaseCheckoutDirty { path })
        }
        Err(e) => block(BlockedReason::MergeFailed {
            detail: e.to_string(),
        }),
    };
    tell_ticket(uow, ticket, event)
}

/// A conflict at Accept: a card with a ship goes back to its agent as a round (the Accept is
/// remembered); a card without one (the sim profile) is blocked with `merge_conflict`.
fn conflict<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    text: String,
) -> AppResult<TicketEvent> {
    Ok(if uow.ship(ticket)?.is_some() {
        TicketEvent::MergeConflicted { text }
    } else {
        block(BlockedReason::MergeConflict { detail: text })
    })
}

/// The base branch the card's work merges into, when it has a project to say.
fn base_of<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: TicketId) -> AppResult<Option<String>> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(None);
    };
    let (Some(chain), Some(ship)) = (uow.chain(t.chain_id)?, uow.ship(ticket)?) else {
        return Ok(None);
    };
    let Some(project) = uow.project(chain.project_id)? else {
        return Ok(None);
    };
    Ok(ShipSpec::for_ticket(&project, &chain, &t)
        .map(|s| s.merge_spec(&t.title, &ship.path).look.start))
}

/// The card's pull request as the host last saw it (an unchanged one changes nothing, so the
/// journal gets no entry for it).
fn record_pr<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    pr: PullRequest,
) -> AppResult<()> {
    let ask_after_secs = match uow.ticket(ticket)? {
        Some(t) => match uow.chain(t.chain_id)? {
            Some(chain) => uow
                .project(chain.project_id)?
                .map(|p| p.merge.no_checks_timeout_secs),
            None => None,
        },
        None => None,
    }
    .unwrap_or(MergeSettings::DEFAULT_NO_CHECKS_TIMEOUT_SECS);
    tell_ticket(uow, ticket, TicketEvent::PrUpdated { pr, ask_after_secs })
}

fn block(reason: BlockedReason) -> TicketEvent {
    TicketEvent::Block { reason }
}
