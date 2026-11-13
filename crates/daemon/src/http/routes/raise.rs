//! Raising and forgetting a worker's permission dialog inside a command (split from `questions`).

use toknila_app::{agent, questions, resolve};
use toknila_core::agent::AgentEvent;
use toknila_core::question::QuestionKind;
use toknila_core::{Actor, AgentRole, Ticket, TicketStatus, Timestamp};

use super::ApiResult;
use crate::server::AppState;
use crate::store::WriteTx;

/// The permission dialog of `role`'s agent, inside a command: a worker's card that is running
/// goes to Waiting with the question; a reviewer's dialog is only journaled.
pub(super) fn ask_person(
    tx: &mut WriteTx<'_>,
    t: &Ticket,
    role: AgentRole,
    text: String,
    detail: Option<String>,
) -> ApiResult<()> {
    if role == AgentRole::Worker {
        if t.status == TicketStatus::Running {
            let event = AgentEvent::AskHuman {
                text,
                permission: true,
                detail,
                choices: Vec::new(),
            };
            agent::ticket_event(tx, t, event)?;
        }
    } else {
        questions::raised(tx, t, role, QuestionKind::Permission, None)?;
    }
    Ok(())
}

/// A question went away without an answer: journal it for the role.
pub(super) async fn forget(
    state: &AppState,
    t: Ticket,
    role: AgentRole,
    at: Timestamp,
) -> ApiResult<()> {
    state
        .store
        .write(Actor::System, at, move |tx| -> ApiResult<()> {
            let t = resolve::ticket(&*tx, &t.key.to_string())?;
            Ok(questions::cleared(tx, &t, role)?)
        })
        .await?;
    Ok(())
}
