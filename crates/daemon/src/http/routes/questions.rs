//! Answering what an agent asks, and the agent's permission hook that waits for the answer.
//!
//! The daemon only detects and delivers: core says what may answer which question and how the
//! answer travels (`toknila_core::question`); this decides nothing of its own. Nothing here
//! writes to a terminal inside a transaction: the answer is delivered first, then journaled.

use crate::http::body::Json;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use toknila_api_types::{
    AnswerOutcome, NewAnswer, PermissionAsk, PermissionDecision, PermissionReply,
};
use toknila_app::{AppError, Reader, agent, projects, questions, resolve, runs};
use toknila_core::agent::AgentEvent;
use toknila_core::command_policy::allow_always_for;
use toknila_core::question::{self, AnswerKind, Decision, Delivery, QuestionKind};
use toknila_core::{
    Actor, AgentRole, StatusReason, Ticket, TicketId, TicketStatus, Timestamp, WaitingReason, hook,
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
    responses((status = 204, description = "Delivered to the agent and journaled"), (status = 200, body = AnswerOutcome, description = "Delivered; the answer was Allow always, and the body says what was added"), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A person's answer to what the ticket's worker or reviewer asks: allow or deny a permission, or
/// a key or text for an agent that waits on its screen. Core says which answers the open question
/// takes; an answer to a permission goes to the agent's waiting hook, else to its terminal. The
/// card of a waiting worker is Running again at once.
pub async fn runner_answer(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(body): Json<NewAnswer>,
) -> ApiResult<Response> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    let role = body.role.unwrap_or(AgentRole::Worker);
    let at = now();
    let runners = state.runners.clone();
    // A paused agent has no dialog or terminal: the answer starts it again.
    if role == AgentRole::Worker
        && (t.agent_paused.is_some() || toknila_app::paused::lost_blocked(&t))
    {
        let (answer, text, card) = (body.answer, body.text.clone(), t.clone());
        let (resumed, _) = state
            .store
            .write(Actor::Human, at, move |tx| {
                toknila_app::paused::answer_permission(tx, &card, answer, text.as_deref())
            })
            .await?;
        if resumed {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
    }
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
    let delivery = if matches!(body.answer, AnswerKind::Yes | AnswerKind::No) {
        let yes = body.answer == AnswerKind::Yes;
        crate::runner::screens::trust_keys(&runners, t.id, role, yes, at)
            .map(Delivery::Keys)
            .ok_or(question::AnswerError::NotOffered(body.answer))
    } else {
        question::deliver(body.answer, body.text.as_deref(), hook_waiting)
    };
    match delivery.map_err(AppError::from)? {
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
    let others = runners.prompts().worker_hook_dialogs();
    let remember_tool = remember.clone();
    let asked_always = body.always && answer == AnswerKind::Allow && role == AgentRole::Worker;
    let ((covered, added), _) = state
        .store
        .write(
            Actor::Human,
            at,
            move |tx| -> ApiResult<(Vec<TicketId>, Option<String>)> {
                let mut covered = Vec::new();
                let mut added = None;
                if let Some(tool) = &remember
                    && let Some(rule) = projects::remember_command(tx, &t, tool)?
                {
                    added = Some(rule);
                    // The new rule also answers what other cards of the project already ask.
                    let project = projects::of_ticket(tx, &t)?;
                    for (id, other_tool) in &others {
                        let Some(other) = tx.ticket(*id)? else {
                            continue;
                        };
                        let same = projects::of_ticket(tx, &other)?.id == project.id;
                        if *id != t.id && same && project.allows_tool(other_tool) {
                            questions::answered(tx, &other, AgentRole::Worker, AnswerKind::Allow)?;
                            covered.push(*id);
                        }
                    }
                }
                questions::answered(tx, &t, role, answer)?;
                Ok((covered, added))
            },
        )
        .await?;
    for id in covered {
        runners
            .prompts()
            .resolve(id, AgentRole::Worker, Decision::Allow);
    }
    if !asked_always {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let note = match &added {
        Some(rule) => format!("Allowed always in this project: {rule}"),
        None => {
            let why = match remember_tool
                .as_deref()
                .and_then(|t| allow_always_for(None, t))
            {
                Some(Err(why)) => why,
                _ => "the question is not about a command",
            };
            format!("Allowed this time only, no rule was added: {why}")
        }
    };
    Ok(Json(AnswerOutcome {
        rule_added: added,
        note,
    })
    .into_response())
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
    let at = now();
    let prompts = state.runners.prompts();
    let tool = ask.tool.or_else(|| prompts.last_tool(t.id, role));
    // A handed-in worker's call and a worker's shell edit are refused without asking anyone.
    if let Some(reply) = automatic_reply(&t, role, tool.as_deref()) {
        return Ok(Json(reply));
    }
    // A command the person allowed always (the project remembers a rule for it) is not asked
    // again, not even by an agent that started before the rule was made. Under trust every tool
    // that reaches this hook is allowed: the tools the deny list names never get this far.
    if role == AgentRole::Worker {
        let tool = tool.clone().unwrap_or_default();
        let ticket = t.clone();
        let covered = state
            .store
            .read(move |repo| {
                Ok(projects::of_ticket(repo, &ticket)
                    .map(|p| p.command_policy.allows_every_tool() || p.allows_tool(&tool)))
            })
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

#[utoipa::path(post, path = "/tickets/{ticket}/pre-tool", tag = "agent", request_body = PermissionAsk,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = PermissionReply), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The agent's `PreToolUse` hook: a tool call is about to run, in any permission mode. Answers with
/// the automatic decisions only (`deny` or `ask`, meaning no decision); never asks a person and
/// never waits.
pub async fn agent_pre_tool(
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
    Ok(Json(
        automatic_reply(&t, role, ask.tool.as_deref()).unwrap_or(PermissionReply {
            decision: PermissionDecision::Ask,
            message: None,
        }),
    ))
}

/// The refusal a tool call gets without asking a person, from core's one function.
fn automatic_reply(t: &Ticket, role: AgentRole, tool: Option<&str>) -> Option<PermissionReply> {
    let scratch = [std::env::temp_dir().to_string_lossy().into_owned()];
    match question::automatic_decision(role, t.status, tool, &scratch)? {
        Decision::Deny { message } => {
            log_refusal(t, role, tool, "automatic decision");
            Some(PermissionReply {
                decision: PermissionDecision::Deny,
                message: Some(message),
            })
        }
        Decision::Allow => None,
    }
}

/// One info line for a request the daemon refused without asking a person, so afterwards it can
/// be seen what the agent tried to run (`why` names the rule that refused it).
fn log_refusal(t: &Ticket, role: AgentRole, tool: Option<&str>, why: &str) {
    tracing::info!(
        ticket = %t.key,
        role = ?role,
        command = tool.unwrap_or("(unknown)"),
        why,
        "refused a permission request without asking"
    );
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
