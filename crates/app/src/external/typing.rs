//! Typing into a live agent's terminal: an instruction for the worker, a nudge for the reviewer.

use toknila_core::instructions::{self, Route};
use toknila_core::{AgentRole, Comment, TicketId};

use crate::runner::Runner;

/// Pause between the typed text and Enter.
pub(super) const ENTER_DELAY: std::time::Duration = std::time::Duration::from_millis(200);

/// Types the instruction into the agent's terminal when that is the way in. Returns whether it
/// was typed. An agent that works with hooks takes it at its next Stop instead, and a ticket
/// without a live runner keeps it pending; neither is an error.
pub(super) fn deliver(runner: &dyn Runner, ticket: TicketId, comment: &Comment) -> bool {
    let Some(state) = runner.state(ticket) else {
        return false;
    };
    if instructions::route(state.hooks, state.idle) != Route::Type {
        return false;
    }
    type_line(
        runner,
        ticket,
        AgentRole::Worker,
        &instructions::typed(comment),
    )
}

/// The text, then Enter as a separate keystroke: a terminal application takes a newline that
/// arrives with the text as part of a paste. Returns whether both were written.
pub(super) fn type_line(
    runner: &dyn Runner,
    ticket: TicketId,
    role: AgentRole,
    line: &str,
) -> bool {
    runner.write(ticket, role, line.as_bytes()).is_ok() && {
        std::thread::sleep(ENTER_DELAY);
        runner.write(ticket, role, b"\r").is_ok()
    }
}
