//! HTTP handlers. Each one resolves references, runs a command in one transaction and answers
//! with the fresh state.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde_json::json;
use toknila_core::graph::ChainView;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, BlockedReason, Chain, ChainId, CommentKind, Edge, EdgeKind, Project, ProjectId, Release,
    ReleaseChannel, ReleaseId, ReleaseState, ReviewMode, StartMode, Ticket, TicketId, TicketKey,
    TicketStatus, Timestamp,
};

use super::commands::{self, SIM_PROFILE, with_chain};
use super::dto::*;
use super::error::{ApiError, ErrorBody};
use crate::server::{AppState, now};
use crate::store::{EventRecord, Repo};
use toknila_core::agent::{AgentEvent, ReleaseEvent};

type ApiResult<T> = Result<T, ApiError>;

fn chain_detail(view: ChainView, at: Timestamp) -> ChainDetail {
    let summary = view.summary(at);
    let order = view.topo_order();
    ChainDetail {
        release_tickets: view
            .release_tickets
            .iter()
            .copied()
            .map(ReleaseMember::from)
            .collect(),
        chain: view.chain,
        tickets: view.tickets,
        releases: view.releases,
        edges: view.edges,
        summary,
        order,
    }
}

fn load_chain_detail(repo: &Repo<'_>, chain: ChainId, at: Timestamp) -> ApiResult<ChainDetail> {
    let view = repo
        .chain_view(chain)?
        .ok_or_else(|| ApiError::not_found(format!("chain {chain}")))?;
    Ok(chain_detail(view, at))
}

fn load_ticket_detail(repo: &Repo<'_>, ticket: TicketId, at: Timestamp) -> ApiResult<TicketDetail> {
    let t = repo
        .ticket(ticket)?
        .ok_or_else(|| ApiError::not_found(format!("ticket {ticket}")))?;
    let chain = repo.chain(t.chain_id)?.map(|c| c.name).unwrap_or_default();
    Ok(TicketDetail {
        subtasks: repo.subtasks(t.id)?,
        comments: repo.comments(t.id)?,
        in_status_ms: at.millis_since(t.status_since),
        chain,
        ticket: t,
    })
}

async fn read<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&Repo<'_>) -> ApiResult<T> + Send + 'static,
) -> ApiResult<T> {
    // Read through the writer: the closure's ApiError rides inside an Ok.

    state
        .store
        .read(move |repo| Ok(f(repo)))
        .await
        .map_err(ApiError::from)?
}

// ---- projects ----

#[utoipa::path(get, path = "/projects", tag = "projects",
    responses((status = 200, body = Vec<Project>)))]
pub async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(read(&state, |repo| Ok(repo.projects()?)).await?))
}

#[utoipa::path(post, path = "/projects", tag = "projects", request_body = NewProject,
    responses((status = 201, body = Project), (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_project(
    State(state): State<AppState>,
    Json(input): Json<NewProject>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let key = input
        .key
        .parse()
        .map_err(|e: toknila_core::KeyError| ApiError::bad_request(e.to_string()))?;
    let p = Project {
        id: ProjectId::new(),
        name: input.name.unwrap_or_else(|| input.key.clone()),
        key,
        repo_path: input.repo_path,
        work_branch: input.work_branch.unwrap_or_else(|| "develop".to_owned()),
        release_branch: input.release_branch.unwrap_or_else(|| "main".to_owned()),
        default_profile: input
            .default_profile
            .unwrap_or_else(|| SIM_PROFILE.to_owned()),
        default_review_mode: input.default_review_mode.unwrap_or(ReviewMode::Human),
    };
    let (p, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Project> {
            tx.repo().insert_project(&p)?;
            tx.emit("project", p.id, "created", json!({ "project": p }))?;
            Ok(p)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(p)))
}

#[utoipa::path(get, path = "/projects/{project}", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Project), (status = 404, body = ErrorBody)))]
pub async fn get_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        read(&state, move |repo| {
            commands::resolve_project(repo, &project)
        })
        .await?,
    ))
}

// ---- chains ----

#[utoipa::path(post, path = "/projects/{project}/chains", tag = "chains", request_body = NewChain,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 201, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_chain(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<NewChain>,
) -> ApiResult<(StatusCode, Json<ChainDetail>)> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request("chain name is empty"));
    }
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<ChainDetail> {
            let p = commands::resolve_project(&tx.repo(), &project)?;
            let c = Chain {
                id: ChainId::new(),
                project_id: p.id,
                name: input.name.trim().to_owned(),
                goal: input.goal.unwrap_or_default(),
                autostart: input.autostart.unwrap_or(true),
                review_mode: input.review_mode,
            };
            tx.repo().insert_chain(&c)?;
            tx.emit("chain", c.id, "created", json!({ "chain": c }))?;
            load_chain_detail(&tx.repo(), c.id, at)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(get, path = "/chains", tag = "chains",
    params(("project" = Option<String>, Query, description = "Project id or key")),
    responses((status = 200, body = Vec<ChainItem>)))]
pub async fn list_chains(
    State(state): State<AppState>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<Vec<ChainItem>>> {
    let at = now();
    let items = read(&state, move |repo| {
        let project = q
            .get("project")
            .map(|p| commands::resolve_project(repo, p))
            .transpose()?;
        let mut out = Vec::new();
        for c in repo.chains(project.map(|p| p.id))? {
            if let Some(v) = repo.chain_view(c.id)? {
                out.push(ChainItem {
                    summary: v.summary(at),
                    chain: c,
                });
            }
        }
        Ok(out)
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(get, path = "/chains/{chain}", tag = "chains",
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody)))]
pub async fn get_chain(
    State(state): State<AppState>,
    Path(chain): Path<String>,
) -> ApiResult<Json<ChainDetail>> {
    let at = now();
    Ok(Json(
        read(&state, move |repo| {
            let c = commands::resolve_chain(repo, &chain)?;
            load_chain_detail(repo, c.id, at)
        })
        .await?,
    ))
}

#[utoipa::path(post, path = "/chains/{chain}/edges", tag = "chains", request_body = EdgeInput,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn add_edge(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    chain_write(&state, chain, move |ctx, _| {
        let from = commands::resolve_node(&ctx.view, &input.from)?;
        let to = commands::resolve_node(&ctx.view, &input.to)?;
        ctx.add_edge(Edge {
            chain_id: ctx.view.chain.id,
            from,
            to,
            kind: input.kind,
        })?;
        // A Ready ticket that now waits again goes back to Backlog when the chain settles.
        Ok(())
    })
    .await
}

#[utoipa::path(delete, path = "/chains/{chain}/edges", tag = "chains",
    params(("chain" = String, Path, description = "Chain id or name"), EdgeInput),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody)))]
pub async fn remove_edge(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Query(input): Query<EdgeInput>,
) -> ApiResult<Json<ChainDetail>> {
    chain_write(&state, chain, move |ctx, _| {
        let from = commands::resolve_node(&ctx.view, &input.from)?;
        let to = commands::resolve_node(&ctx.view, &input.to)?;
        ctx.remove_edge(from, to)
            .map(|_| ())
            .ok_or_else(|| ApiError::not_found(format!("edge {} -> {}", input.from, input.to)))
    })
    .await
}

#[utoipa::path(post, path = "/chains/{chain}/releases", tag = "releases", request_body = NewRelease,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_release(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewRelease>,
) -> ApiResult<Json<ChainDetail>> {
    if input.version.trim().is_empty() {
        return Err(ApiError::bad_request("release version is empty"));
    }
    chain_write(&state, chain, move |ctx, _| {
        if ctx.view.releases.iter().any(|r| r.version == input.version) {
            return Err(ApiError::conflict(format!(
                "release {} already exists",
                input.version
            )));
        }
        let r = Release {
            id: ReleaseId::new(),
            chain_id: ctx.view.chain.id,
            version: input.version.trim().to_owned(),
            channel: input.channel.unwrap_or(ReleaseChannel::TagOnly),
            start_mode: input.start_mode.unwrap_or(StartMode::Manual),
            state: ReleaseState::Planned,
            branch: None,
            cut_at: None,
            released_at: None,
        };
        let node = toknila_core::NodeRef::Release(r.id);
        ctx.add_release(r);
        let chain_id = ctx.view.chain.id;
        for a in &input.after {
            let from = commands::resolve_node(&ctx.view, a)?;
            ctx.add_edge(Edge {
                chain_id,
                from,
                to: node,
                kind: EdgeKind::Normal,
            })?;
        }
        for b in &input.before {
            let to = commands::resolve_node(&ctx.view, b)?;
            ctx.add_edge(Edge {
                chain_id,
                from: node,
                to,
                kind: EdgeKind::Normal,
            })?;
        }
        Ok(())
    })
    .await
}

async fn chain_write(
    state: &AppState,
    chain: String,
    f: impl FnOnce(&mut commands::ChainTx, &Repo<'_>) -> ApiResult<()> + Send + 'static,
) -> ApiResult<Json<ChainDetail>> {
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<ChainDetail> {
            let c = commands::resolve_chain(&tx.repo(), &chain)?;
            with_chain(tx, c.id, f)?;
            load_chain_detail(&tx.repo(), c.id, at)
        })
        .await?;
    Ok(Json(detail))
}

// ---- tickets ----

#[utoipa::path(post, path = "/chains/{chain}/tickets", tag = "tickets", request_body = NewTicket,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 201, body = TicketDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_ticket(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewTicket>,
) -> ApiResult<(StatusCode, Json<TicketDetail>)> {
    if input.title.trim().is_empty() {
        return Err(ApiError::bad_request("ticket title is empty"));
    }
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<TicketDetail> {
            let c = commands::resolve_chain(&tx.repo(), &chain)?;
            let project = tx
                .repo()
                .project(c.project_id)?
                .ok_or_else(|| ApiError::not_found("project"))?;
            let number = tx.repo().take_ticket_number(project.id)?;
            let key = TicketKey::new(project.key.clone(), number)
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            let t = Ticket {
                id: TicketId::new(),
                chain_id: c.id,
                key,
                title: input.title.trim().to_owned(),
                prompt: input.prompt,
                acceptance: input.acceptance,
                status: TicketStatus::Backlog,
                status_reason: None,
                status_since: at,
                paused: input.paused,
                profile: input.profile,
                review_mode: input.review_mode,
                branch: None,
                needs_secrets: input.needs_secrets,
                tokens_used: 0,
            };
            let id = t.id;
            let after = input.after;
            let subtasks = input.subtasks;
            // The ticket and its edges land together, then the chain settles (autostart).
            with_chain(tx, c.id, move |ctx, _| {
                let deps = after
                    .iter()
                    .map(|a| commands::resolve_node(&ctx.view, a))
                    .collect::<ApiResult<Vec<_>>>()?;
                ctx.add_ticket(t);
                for from in deps {
                    ctx.add_edge(Edge {
                        chain_id: ctx.view.chain.id,
                        from,
                        to: toknila_core::NodeRef::Ticket(id),
                        kind: EdgeKind::Normal,
                    })?;
                }
                Ok(())
            })?;
            let ticket = tx
                .repo()
                .ticket(id)?
                .ok_or_else(|| ApiError::not_found("ticket"))?;
            for s in &subtasks {
                commands::add_subtask(tx, &ticket, s)?;
            }
            load_ticket_detail(&tx.repo(), id, at)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(get, path = "/tickets", tag = "tickets", params(TicketQuery),
    responses((status = 200, body = Vec<TicketItem>)))]
pub async fn list_tickets(
    State(state): State<AppState>,
    Query(q): Query<TicketQuery>,
) -> ApiResult<Json<Vec<TicketItem>>> {
    let at = now();
    let items = read(&state, move |repo| {
        let chain = q
            .chain
            .as_deref()
            .map(|c| commands::resolve_chain(repo, c))
            .transpose()?;
        let names: std::collections::HashMap<ChainId, String> = repo
            .chains(None)?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect();
        Ok(repo
            .tickets(chain.map(|c| c.id))?
            .into_iter()
            .filter(|t| !q.needs_human || t.status.needs_human())
            .map(|t| TicketItem {
                chain: names.get(&t.chain_id).cloned().unwrap_or_default(),
                in_status_ms: at.millis_since(t.status_since),
                needs_human: t.status.needs_human(),
                ticket: t,
            })
            .collect())
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(get, path = "/tickets/{ticket}", tag = "tickets",
    params(("ticket" = String, Path, description = "Ticket id or key, e.g. DEMO-3")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn get_ticket(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<TicketDetail>> {
    let at = now();
    Ok(Json(
        read(&state, move |repo| {
            let t = commands::resolve_ticket(repo, &ticket)?;
            load_ticket_detail(repo, t.id, at)
        })
        .await?,
    ))
}

#[utoipa::path(patch, path = "/tickets/{ticket}", tag = "tickets", request_body = PatchTicket,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn patch_ticket(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(p): Json<PatchTicket>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        with_chain(tx, t.chain_id, |ctx, _| {
            Ok(ctx.edit_ticket(t.id, |t| {
                if let Some(v) = p.title {
                    t.title = v;
                }
                if let Some(v) = p.prompt {
                    t.prompt = v;
                }
                if let Some(v) = p.acceptance {
                    t.acceptance = v;
                }
                if let Some(v) = p.profile {
                    t.profile = Some(v);
                }
                if let Some(v) = p.review_mode {
                    t.review_mode = Some(v);
                }
                if let Some(v) = p.needs_secrets {
                    t.needs_secrets = v;
                }
            })?)
        })
    })
    .await
}

/// Runs `f` for one ticket in a transaction and answers with the ticket's fresh detail.
pub(crate) async fn ticket_write(
    state: &AppState,
    ticket: String,
    actor: Actor,
    f: impl FnOnce(&mut crate::store::WriteTx<'_>, Ticket) -> ApiResult<()> + Send + 'static,
) -> ApiResult<Json<TicketDetail>> {
    let at = now();
    let (detail, _) = state
        .store
        .write(actor, at, move |tx| -> ApiResult<TicketDetail> {
            let t = commands::resolve_ticket(&tx.repo(), &ticket)?;
            let id = t.id;
            f(tx, t)?;
            load_ticket_detail(&tx.repo(), id, at)
        })
        .await?;
    Ok(Json(detail))
}

fn required_text(input: &ActionInput, action: &str) -> ApiResult<String> {
    input
        .text
        .clone()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("{action} needs a text")))
}

#[utoipa::path(post, path = "/tickets/{ticket}/actions/{action}", tag = "tickets", request_body = ActionInput,
    params(
        ("ticket" = String, Path, description = "Ticket id or key"),
        ("action" = String, Path, description = "start, pause, resume, block, unblock, accept, return, reopen or cancel"),
    ),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn ticket_action(
    State(state): State<AppState>,
    Path((ticket, action)): Path<(String, String)>,
    body: Option<Json<ActionInput>>,
) -> ApiResult<Json<TicketDetail>> {
    let input = body.map(|Json(b)| b).unwrap_or_default();
    let event = match action.as_str() {
        "start" => TicketEvent::Start,
        "pause" => TicketEvent::Pause,
        "resume" => TicketEvent::Resume,
        "block" => TicketEvent::Block {
            reason: BlockedReason::Manual {
                text: required_text(&input, "block")?,
            },
        },
        "unblock" => TicketEvent::Unblock,
        "accept" => TicketEvent::Accept,
        "return" => TicketEvent::Return {
            comment: required_text(&input, "return")?,
        },
        "reopen" => TicketEvent::Reopen,
        "cancel" => TicketEvent::Cancel {
            reason: required_text(&input, "cancel")?,
        },
        other => return Err(ApiError::not_found(format!("action {other}"))),
    };
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        with_chain(tx, t.chain_id, |ctx, _| Ok(ctx.ticket_event(t.id, event)?))
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/comments", tag = "tickets", request_body = NewComment,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
pub async fn add_comment(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<NewComment>,
) -> ApiResult<Json<TicketDetail>> {
    let body = match (input.body, input.text) {
        (Some(b), _) => b,
        (None, Some(text)) => commands::body_from_text(&text),
        (None, None) => return Err(ApiError::bad_request("give text or body")),
    };
    let kind = input.kind.unwrap_or(CommentKind::Instruction);
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        let view = tx
            .repo()
            .chain_view(t.chain_id)?
            .ok_or_else(|| ApiError::not_found("chain"))?;
        commands::add_comment(tx, &view, t.id, Actor::Human, kind, body)?;
        // Delivery to a simulated agent happens as the chain settles.
        with_chain(tx, t.chain_id, |_, _| Ok(()))
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/subtasks", tag = "tickets", request_body = NewSubtask,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn add_subtask(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<NewSubtask>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        commands::add_subtask(tx, &t, &input.text).map(|_| ())
    })
    .await
}

#[utoipa::path(patch, path = "/tickets/{ticket}/subtasks/{position}", tag = "tickets", request_body = PatchSubtask,
    params(("ticket" = String, Path, description = "Ticket id or key"), ("position" = u32, Path, description = "1-based position")),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody)))]
pub async fn patch_subtask(
    State(state): State<AppState>,
    Path((ticket, position)): Path<(String, u32)>,
    Json(input): Json<PatchSubtask>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        commands::update_subtask(tx, &t, position, input.done, input.text).map(|_| ())
    })
    .await
}

// ---- releases ----

#[utoipa::path(post, path = "/releases/{release}/actions/{action}", tag = "releases",
    params(("release" = String, Path, description = "Release id or version"), ("action" = String, Path, description = "start (cut) or complete (released)")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn release_action(
    State(state): State<AppState>,
    Path((release, action)): Path<(String, String)>,
) -> ApiResult<Json<ChainDetail>> {
    if !matches!(action.as_str(), "start" | "complete") {
        return Err(ApiError::not_found(format!("action {action}")));
    }
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Human, at, move |tx| -> ApiResult<ChainDetail> {
            let r = commands::resolve_release(&tx.repo(), &release)?;
            with_chain(tx, r.chain_id, |ctx, _| {
                if action == "start" {
                    ctx.cut_release(r.id)?
                } else {
                    ctx.complete_release(r.id)?
                }
                Ok(())
            })?;
            load_chain_detail(&tx.repo(), r.chain_id, at)
        })
        .await?;
    Ok(Json(detail))
}

// ---- events ----

#[utoipa::path(get, path = "/events", tag = "events", params(EventQuery),
    responses((status = 200, body = Vec<EventRecord>)))]
pub async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<EventQuery>,
) -> ApiResult<Json<Vec<EventRecord>>> {
    let limit = q.limit.unwrap_or(1000).min(10_000);
    Ok(Json(state.store.events_since(q.since, limit).await?))
}

// ---- agent events (P0-10) ----

#[utoipa::path(post, path = "/tickets/{ticket}/events", tag = "agent", request_body = AgentEvent,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 200, body = TicketDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn agent_event(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(event): Json<AgentEvent>,
) -> ApiResult<Json<TicketDetail>> {
    ticket_write(&state, ticket, Actor::Agent, move |tx, t| match event {
        AgentEvent::Started => with_chain(tx, t.chain_id, |ctx, _| {
            if t.status == TicketStatus::Ready {
                ctx.ticket_event(t.id, TicketEvent::Start)?;
            } else if t.status != TicketStatus::Running {
                return Err(ApiError::conflict(format!(
                    "cannot start a ticket in {:?}",
                    t.status
                )));
            }
            Ok(())
        }),
        AgentEvent::AskHuman { text, permission } => {
            if text.trim().is_empty() {
                return Err(ApiError::bad_request("ask_human needs a text"));
            }
            let reason = AgentEvent::waiting_reason(&text, permission);
            with_chain(tx, t.chain_id, |ctx, _| {
                Ok(ctx.ticket_event(t.id, TicketEvent::AskHuman { reason })?)
            })?;
            let view = tx
                .repo()
                .chain_view(t.chain_id)?
                .ok_or_else(|| ApiError::not_found("chain"))?;
            commands::add_comment(
                tx,
                &view,
                t.id,
                Actor::Agent,
                CommentKind::Report,
                commands::body_from_text(&text),
            )
            .map(|_| ())
        }
        AgentEvent::SubtaskDone { position } => {
            commands::update_subtask(tx, &t, position, Some(true), None).map(|_| ())
        }
        AgentEvent::Note { body } => {
            let view = tx
                .repo()
                .chain_view(t.chain_id)?
                .ok_or_else(|| ApiError::not_found("chain"))?;
            commands::add_comment(tx, &view, t.id, Actor::Agent, CommentKind::Report, body)
                .map(|_| ())
        }
        AgentEvent::Finished { summary } => {
            if summary.trim().is_empty() {
                return Err(ApiError::bad_request("finished needs a summary"));
            }
            with_chain(tx, t.chain_id, |ctx, _| {
                Ok(ctx.ticket_event(t.id, TicketEvent::Finished { summary })?)
            })
        }
        AgentEvent::Crashed { detail } => with_chain(tx, t.chain_id, |ctx, _| {
            Ok(ctx.ticket_event(
                t.id,
                TicketEvent::Block {
                    reason: BlockedReason::Crashed { detail },
                },
            )?)
        }),
    })
    .await
}

#[utoipa::path(post, path = "/releases/{release}/events", tag = "agent", request_body = ReleaseEvent,
    params(("release" = String, Path, description = "Release id or version")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn release_event(
    State(state): State<AppState>,
    Path(release): Path<String>,
    Json(event): Json<ReleaseEvent>,
) -> ApiResult<Json<ChainDetail>> {
    let at = now();
    let (detail, _) = state
        .store
        .write(Actor::Agent, at, move |tx| -> ApiResult<ChainDetail> {
            let r = commands::resolve_release(&tx.repo(), &release)?;
            match event {
                ReleaseEvent::Finished => {
                    with_chain(tx, r.chain_id, |ctx, _| Ok(ctx.complete_release(r.id)?))?
                }
            }
            load_chain_detail(&tx.repo(), r.chain_id, at)
        })
        .await?;
    Ok(Json(detail))
}
