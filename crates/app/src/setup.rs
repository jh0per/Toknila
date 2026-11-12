//! The project's setup command: the host runs it once in a new ship before the card's agent
//! starts ([`crate::external`]); this module records how it ended on the card, as a notice by
//! Toknila, and blocks the card when it failed, so no agent starts on a workspace that is not
//! ready.

use toknila_core::machine::TicketEvent;
use toknila_core::{Actor, BlockedReason, CommentBody, CommentKind, TicketId};

use crate::chain::tell_ticket;
use crate::checks::{CheckError, CheckResult, CheckSpec, report};
use crate::external::{ExternalOutcome, Host};
use crate::notes::add_comment;
use crate::{AppResult, UnitOfWork};

/// How many lines of the output the blocked reason keeps (the end of it, where the failure is).
const LAST_LINES: usize = 20;

/// Runs the setup command in the ship at `dir` unless it already ran there. `None`: nothing to
/// do (no command, or the ship has its mark); otherwise the outcome and whether the agent may
/// start. A success marks the ship.
pub(crate) fn run_on_host(
    host: &Host<'_>,
    ticket: TicketId,
    command: Option<&str>,
    dir: &str,
) -> Option<(ExternalOutcome, bool)> {
    let command = command?;
    if host.yard.setup_ran(dir) {
        return None;
    }
    let spec = CheckSpec {
        command: command.to_owned(),
        dir: dir.to_owned(),
    };
    let result = host.checker.run(&spec);
    let ok = succeeded(&result);
    if ok {
        host.yard.mark_setup_ran(dir);
    }
    let command = spec.command;
    let run = SetupRun {
        ticket,
        command,
        result,
    };
    Some((ExternalOutcome::SetupFinished(run), ok))
}

/// A setup command that ran in a ship (or could not run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRun {
    pub ticket: TicketId,
    pub command: String,
    pub result: Result<CheckResult, CheckError>,
}

/// Whether the outcome lets the agent start.
fn succeeded(result: &Result<CheckResult, CheckError>) -> bool {
    result.as_ref().is_ok_and(|r| r.success)
}

/// The last [`LAST_LINES`] lines of `text`.
fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    lines[lines.len().saturating_sub(LAST_LINES)..].join("\n")
}

/// The setup ended: the card gets a notice with the command and the end of its output, and a
/// failure blocks it with `setup failed: <command>` and the last lines.
pub(crate) fn finished<U: UnitOfWork + ?Sized>(uow: &mut U, run: SetupRun) -> AppResult<()> {
    let SetupRun {
        ticket,
        command,
        result,
    } = run;
    let command = command.as_str();
    let ok = succeeded(&result);
    let (what, output) = match &result {
        Ok(r) => (report(command, r), last_lines(&r.output)),
        Err(e) => (format!("`{command}` could not run: {e}"), e.to_string()),
    };
    if let Some(t) = uow.ticket(ticket)?
        && let Some(view) = uow.chain_view(t.chain_id)?
    {
        let gist = if ok {
            format!("Setup ran: {command}")
        } else {
            format!("Setup failed: {command}")
        };
        let body = CommentBody {
            gist,
            what: Some(what),
            ..CommentBody::default()
        };
        add_comment(uow, &view, ticket, Actor::System, CommentKind::Report, body)?;
    }
    if ok {
        return Ok(());
    }
    let reason = BlockedReason::SetupFailed {
        command: command.to_owned(),
        output,
    };
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
}
