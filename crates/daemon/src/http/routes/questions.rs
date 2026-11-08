//! Answering what an agent asks, and the agent's permission hook that waits for the answer.
//!
//! The daemon only detects and delivers: core says what may answer which question and how the
//! answer travels (`toknila_core::question`); this decides nothing of its own. Nothing here
//! writes to a terminal inside a transaction: the answer is delivered first, then journaled.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use toknila_api_types::{NewAnswer, PermissionAsk, PermissionDecision, PermissionReply};
use toknila_app::{AppError, agent, projects, questions, resolve, runs};
use toknila_core::agent::AgentEvent;
use toknila_core::question::{self, AnswerKind, Decision, Delivery, QuestionKind};
use toknila_core::{
    Actor, AgentRole, StatusReason, Ticket, TicketStatus, Timestamp, WaitingReason, hook,
};

use super::ApiResult;
use crate::http::error::ErrorBody;
use crate::runner::Runners;
use crate::server::{AppState, Principal, now};
use crate::store::WriteTx;

/// The question the run of `role` has open for a person, if any.
fn open_question(
    runners: &Runners,
    t: &Ticket,
    role: AgentRole,
    at: Timestamp,
) -> Option<QuestionKind> {
    if runners.prompts().is_pending(t.id, role) {
        return Some(QuestionKind::Permission);
    }
    if role == AgentRole::Worker {
        match &t.status_reason {
            Some(StatusReason::Waiting(WaitingReason::Permission { .. })) => {
                return Some(QuestionKind::Permission);
            }
            Some(StatusReason::Waiting(WaitingReason::Question { .. })) => {
                return Some(QuestionKind::Question);
            }
            _ => {}
        }
    }
    let live = runners.facts(at);
    live.runs
        .iter()
        .any(|r| r.ticket == t.id && r.role == role && r.waits_on_screen(at))
        .then_some(QuestionKind::Screen)
}

#[utoipa::path(post, path = "/tickets/{ticket}/runner/answer", tag = "runner",
    params(("ticket" = String, Path, description = "Ticket id or key")),
    request_body = NewAnswer,
    responses((status = 204, description = "Delivered to the agent and journaled"), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A person's answer to what the ticket's worker or reviewer asks: allow or deny a permission, or
/// a key or text for an agent that waits on its screen. Core says which answers the open question
/// takes; an answer to a permission goes to the agent's waiting hook, else to its terminal. The
/// card of a waiting worker is Running again at once.
pub async fn runner_answer(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(body): Json<NewAnswer>,
) -> ApiResult<StatusCode> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    let role = body.role.unwrap_or(AgentRole::Worker);
    let at = now();
    let runners = state.runners.clone();
    let Some(kind) = open_question(&runners, &t, role, at) else {
        return Err(AppError::conflict(format!("the {role:?} of {} asks nothing", t.key)).into());
    };
    if !kind.accepts(body.answer) {
        return Err(AppError::from(question::AnswerError::NotOffered(body.answer)).into());
    }
    // "Allow always": the Bash call the dialog is about, taken before the answer clears it.
    let remember = (body.always && body.answer == AnswerKind::Allow && role == AgentRole::Worker)
        .then(|| {
            let prompts = runners.prompts();
            prompts
                .dialog_tool(t.id, role)
                .or_else(|| prompts.last_tool(t.id, role))
        })
        .flatten();
    let hook_waiting = runners.prompts().hook_waiting(t.id, role);
    match question::deliver(body.answer, body.text.as_deref(), hook_waiting)
        .map_err(AppError::from)?
    {
        Delivery::Hook(decision) => {
            if !runners.prompts().resolve(t.id, role, decision) {
                return Err(AppError::conflict("the agent no longer waits for a decision").into());
            }
        }
        Delivery::Keys(bytes) => {
            // The screen the answer is given to, taken before the keys change it.
            let screen = runners.screen_answer(t.id, role, at);
            runs::send_input(&*runners, Actor::Human, &t, role, &bytes)?;
            runners.prompts().clear(t.id, role);
            runners.prompts().unflag_screen(t.id, role);
            runners.tokens().rewait(t.id, role, screen);
        }
    }
    let answer = body.answer;
    state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<()> {
            if let Some(tool) = &remember {
                projects::remember_command(tx, &t, tool)?;
            }
            Ok(questions::answered(tx, &t, role, answer)?)
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/tickets/{ticket}/permission", tag = "agent", request_body = PermissionAsk,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = PermissionReply), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The agent's `PermissionRequest` hook: a permission dialog is about to show. The worker's card
/// waits for the person (a reviewer's dialog is listed in the inbox), and the call is held until
/// the person allows or denies in the window, up to the daemon's wait; then the agent shows its
/// own dialog (`decision: ask`).
pub async fn agent_permission(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ticket): Path<String>,
    Json(ask): Json<PermissionAsk>,
) -> ApiResult<Json<PermissionReply>> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    principal.owns(&t)?;
    let role = match principal {
        Principal::Agent(grant) => grant.role,
        Principal::Person => AgentRole::Worker,
    };
    // A worker that handed in and still wants to run something is refused without asking anyone.
    if let Some(Decision::Deny { message }) = question::auto_decision(role, t.status) {
        return Ok(Json(PermissionReply {
            decision: PermissionDecision::Deny,
            message: Some(message),
        }));
    }
    let at = now();
    let prompts = state.runners.prompts();
    let tool = ask.tool.or_else(|| prompts.last_tool(t.id, role));
    // A command the person allowed always (the project remembers a rule for it) is not asked
    // again, not even by an agent that started before the rule was made.
    if role == AgentRole::Worker
        && let Some(tool) = tool.clone()
    {
        let ticket = t.clone();
        let covered = state
            .store
            .read(move |repo| Ok(projects::of_ticket(repo, &ticket).map(|p| p.allows_tool(&tool))))
            .await??;
        if covered {
            return Ok(Json(PermissionReply {
                decision: PermissionDecision::Allow,
                message: None,
            }));
        }
    }
    let (summary, detail) = tool.clone().map_or((None, None), |full| {
        let (summary, detail) = hook::summary_and_detail(&full);
        (Some(summary), detail)
    });
    let text = hook::permission_text(None, summary.as_deref());
    let (_, rx) = prompts.begin(t.id, role, text.clone(), detail.clone(), at, true);
    if let Some(tool) = tool {
        prompts.note_dialog_tool(t.id, role, tool);
    }
    raise(&state, t.clone(), role, (text, detail), at).await?;
    let reply = match rx {
        Some(rx) => match tokio::time::timeout(state.runners.permission_wait(), rx).await {
            Ok(Ok(Decision::Allow)) => PermissionReply {
                decision: PermissionDecision::Allow,
                message: None,
            },
            Ok(Ok(Decision::Deny { message })) => PermissionReply {
                decision: PermissionDecision::Deny,
                message: Some(message),
            },
            _ => {
                // Nobody answered in time: the agent shows its own dialog, which the person can
                // still answer through the terminal.
                if prompts.clear(t.id, role) {
                    forget(&state, t, role, now()).await?;
                }
                PermissionReply {
                    decision: PermissionDecision::Ask,
                    message: None,
                }
            }
        },
        None => PermissionReply {
            decision: PermissionDecision::Ask,
            message: None,
        },
    };
    Ok(Json(reply))
}

/// Shows the dialog to the person: the worker's card asks (Running to Waiting), a reviewer's
/// dialog is journaled so clients refresh their inbox.
pub(super) async fn raise(
    state: &AppState,
    t: Ticket,
    role: AgentRole,
    (text, detail): (String, Option<String>),
    at: Timestamp,
) -> ApiResult<()> {
    state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<()> {
            let t = resolve::ticket(&*tx, &t.key.to_string())?;
            ask_person(tx, &t, role, text, detail)
        })
        .await?;
    Ok(())
}

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
            };
            agent::ticket_event(tx, t, event)?;
        }
    } else {
        questions::raised(tx, t, role, QuestionKind::Permission)?;
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
