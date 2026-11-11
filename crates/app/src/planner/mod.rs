//! The planner: a project's main agent, as use cases. Each command here is one the planner's MCP
//! tools call, scoped to the planner's own project: a chain or card of another project is
//! refused (`403`), and so is anything that could start a card. It goes through the same use
//! cases as the window and the CLI (`chains`, `tickets`, `threads`), so core's rules apply; this
//! module adds the scope, and `toknila_core::planner` holds the rules of what a planner may do.
//!
//! The planner's own run (start, stop, input, size) is here too: a project has at most one.

mod plan;
mod window;
mod with_designs;

use serde::{Deserialize, Serialize};
use toknila_core::instructions::ProjectFacts;
use toknila_core::machine::person_only;
use toknila_core::planner::{self, Refusal};
use toknila_core::{
    Actor, AgentProfile, AgentRole, BaseBehind, Chain, ChainId, EdgeKind, PlannerSite, Project,
    ProjectId, Thread, ThreadKind, Ticket, TicketId, TicketKey, TicketPatch,
};

pub use plan::{PlanCard, PlanChain, PlanThread, PlanView, plan};
pub use window::show_card;
pub use with_designs::{request_with, start_with};

use crate::runner::{RunBrief, RunRequest, Runner, RunnerError, Started};
use crate::tickets;
use crate::tickets::NewTicket;
use crate::{AppError, AppResult, ErrorKind, Reader, UnitOfWork, chains, notes, resolve, threads};

impl From<Refusal> for AppError {
    fn from(r: Refusal) -> Self {
        let kind = match r {
            Refusal::OtherProject { .. }
            | Refusal::AutostartChain { .. }
            | Refusal::CardStarted { .. } => ErrorKind::Forbidden,
            Refusal::NoRepository(_) | Refusal::Archived(_) => ErrorKind::Conflict,
        };
        AppError::new(kind, r.to_string())
    }
}

/// A chain of the planner's project by id or name. A chain of another project is refused, one
/// that does not exist is not found.
pub fn chain<R: Reader + ?Sized>(r: &R, project: ProjectId, reference: &str) -> AppResult<Chain> {
    if let Ok(id) = reference.parse::<ChainId>()
        && let Some(c) = r.chain(id)?
    {
        return in_project(r, project, c);
    }
    if let Some(c) = r.chain_by_name(project, reference)? {
        return Ok(c);
    }
    // A name that only another project has is that project's chain.
    match r.chains_named(reference)?.into_iter().next() {
        Some(c) => in_project(r, project, c),
        None => Err(AppError::not_found(format!("chain {reference}"))),
    }
}

fn in_project<R: Reader + ?Sized>(r: &R, project: ProjectId, c: Chain) -> AppResult<Chain> {
    let owner = r
        .project(c.project_id)?
        .map_or_else(String::new, |p| p.key.to_string());
    planner::ensure_in_project(project, c.project_id, format!("chain {}", c.name), &owner)?;
    Ok(c)
}

/// A card of the planner's project by id, key or title.
pub fn card<R: Reader + ?Sized>(r: &R, project: ProjectId, reference: &str) -> AppResult<Ticket> {
    let t = resolve::ticket(r, reference)?;
    let chain = r
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("chain of {}", t.key)))?;
    let owner = r
        .project(chain.project_id)?
        .map_or_else(String::new, |p| p.key.to_string());
    planner::ensure_in_project(project, chain.project_id, format!("card {}", t.key), &owner)?;
    Ok(t)
}

/// What the planner may give for a new chain; a chain it makes never starts cards by itself.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewPlanChain {
    pub name: String,
    pub goal: Option<String>,
}

/// A new chain in the planner's project.
pub fn new_chain<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    input: NewPlanChain,
) -> AppResult<ChainId> {
    let input = chains::NewChain {
        name: input.name,
        goal: input.goal,
        autostart: Some(planner::NEW_CHAIN_AUTOSTART),
        review_mode: None,
        reviewer: None,
    };
    chains::create(uow, &project.to_string(), input)
}

/// What the planner gives for a new card.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewCard {
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Cards (keys) or release versions of the chain it waits for. Empty: the end of the chain.
    #[serde(default)]
    pub after: Vec<String>,
    /// Checklist items.
    #[serde(default)]
    pub subtasks: Vec<String>,
    /// The card changes what the person sees: the agent shows a design for approval before any
    /// code.
    #[serde(default)]
    pub design_first: bool,
}

/// Adds a card to a chain of the planner's project.
pub fn add_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    input: NewCard,
) -> AppResult<TicketId> {
    let c = chain(uow, project, chain_ref)?;
    planner::ensure_graph_change(&c)?;
    let at_end = input.after.is_empty();
    let input = NewTicket {
        title: input.title,
        prompt: input.prompt,
        acceptance: input.acceptance,
        after: input.after,
        subtasks: input.subtasks,
        design_first: input.design_first,
        at_end,
        ..NewTicket::default()
    };
    tickets::create(uow, &c.id.to_string(), input)
}

/// A rename of a checklist item.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubtaskEdit {
    /// 1-based position.
    pub position: u32,
    pub text: String,
}

/// What the planner may change on a card that has not started.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CardEdit {
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub acceptance: Option<Vec<String>>,
    /// Whether the agent shows a design for approval before any code.
    pub design_first: Option<bool>,
    /// Checklist items to append.
    #[serde(default)]
    pub add_subtasks: Vec<String>,
    /// Checklist items to rename.
    #[serde(default)]
    pub edit_subtasks: Vec<SubtaskEdit>,
}

/// Edits a card of the planner's project that has not started.
pub fn edit_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    card_ref: &str,
    edit: CardEdit,
) -> AppResult<TicketId> {
    let t = card(uow, project, card_ref)?;
    planner::ensure_card_editable(&t)?;
    let patch = TicketPatch {
        title: edit.title,
        prompt: edit.prompt,
        acceptance: edit.acceptance,
        design_first: edit.design_first,
        ..TicketPatch::default()
    };
    if patch != TicketPatch::default() {
        tickets::edit(uow, &t, patch)?;
    }
    for item in &edit.edit_subtasks {
        notes::update_subtask(uow, &t, item.position, None, Some(item.text.clone()))?;
    }
    for text in &edit.add_subtasks {
        notes::add_subtask(uow, &t, text)?;
    }
    Ok(t.id)
}

/// Makes `to` wait for `from` in a chain of the planner's project.
pub fn add_dependency<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    from: &str,
    to: &str,
) -> AppResult<ChainId> {
    let c = chain(uow, project, chain_ref)?;
    planner::ensure_graph_change(&c)?;
    chains::add_edge(uow, &c.id.to_string(), from, to, EdgeKind::Normal)
}

/// Removes the wait of `to` for `from`.
pub fn remove_dependency<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    from: &str,
    to: &str,
) -> AppResult<ChainId> {
    let c = chain(uow, project, chain_ref)?;
    planner::ensure_graph_change(&c)?;
    chains::remove_edge(uow, &c.id.to_string(), from, to)
}

/// Puts a card directly before or after another in a chain of the planner's project.
pub fn move_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    card_ref: &str,
    anchor: &str,
    place: toknila_core::graph::Place,
) -> AppResult<ChainId> {
    let c = chain(uow, project, chain_ref)?;
    planner::ensure_graph_change(&c)?;
    chains::move_ticket(uow, &c.id.to_string(), card_ref, anchor, place)
}

/// Opens a thread on a card of the planner's project.
pub fn open_thread<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    card_ref: &str,
    kind: ThreadKind,
    title: &str,
) -> AppResult<(Ticket, Thread)> {
    let t = card(uow, project, card_ref)?;
    let th = threads::open(uow, &t, kind, title)?;
    Ok((t, th))
}

/// A thread of a card of the planner's project, by number or id.
pub fn thread<R: Reader + ?Sized>(
    r: &R,
    project: ProjectId,
    card_ref: &str,
    thread_ref: &str,
) -> AppResult<(Ticket, Thread)> {
    let t = card(r, project, card_ref)?;
    let th = threads::find(r, &t, thread_ref)?;
    Ok((t, th))
}

/// What a project's planner run is told about the project.
fn facts(p: &Project, behind: Option<BaseBehind>) -> ProjectFacts {
    ProjectFacts {
        key: p.key.to_string(),
        name: p.name.clone(),
        work_branch: p.work_branch.clone(),
        merge_mode: p.merge.mode,
        merge_method: p.merge.method,
        check_command: p.check_command.clone(),
        base_behind: behind,
    }
}

/// The request that starts `project`'s planner with the profile named `profile`, in the
/// project's repository. The caller has checked that the profile's role is planner.
pub fn request(project: &Project, profile: &AgentProfile) -> AppResult<RunRequest> {
    request_at(project, profile, None)
}

/// [`request`] for a planner that runs in `site`, which the host prepared; `None`: the project's
/// repository as it is.
pub fn request_at(
    project: &Project,
    profile: &AgentProfile,
    site: Option<PlannerSite>,
) -> AppResult<RunRequest> {
    planner::ensure_can_run(project)?;
    if profile.role != AgentRole::Planner {
        return Err(AppError::bad_request(format!(
            "agent profile {} is not a planner profile",
            profile.name
        )));
    }
    let (dir, behind) = match site {
        Some(s) => (s.dir, s.behind),
        None => (project.repo_path.clone().unwrap_or_default(), None),
    };
    Ok(RunRequest {
        ticket: planner::run_key(project.id),
        key: TicketKey::planner(project.key.clone()),
        role: AgentRole::Planner,
        profile: profile.name.clone(),
        dir,
        brief: RunBrief {
            title: project.name.clone(),
            check_command: project.check_command.clone(),
            model: project.planner_model.clone(),
            project: Some(facts(project, behind)),
            ..RunBrief::default()
        },
    })
}

fn runner_error(e: RunnerError) -> AppError {
    let kind = match e {
        RunnerError::NotRunning => ErrorKind::Conflict,
        RunnerError::UnknownProfile(_) | RunnerError::NoCommand(_) => ErrorKind::BadRequest,
        _ => ErrorKind::Internal,
    };
    AppError::new(kind, e.to_string())
}

/// Starts the project's planner, or keeps the live one. A person's action.
pub fn start(
    runner: &dyn Runner,
    actor: Actor,
    project: &Project,
    profile: &AgentProfile,
    site: Option<PlannerSite>,
) -> AppResult<Started> {
    person_only(actor, "start the planner")?;
    runner
        .start(&request_at(project, profile, site)?)
        .map_err(runner_error)
}

/// Stops the project's planner; none live is fine. A person's action.
pub fn stop(runner: &dyn Runner, actor: Actor, project: &Project) -> AppResult<()> {
    person_only(actor, "stop the planner")?;
    runner.stop_role(planner::run_key(project.id), AgentRole::Planner);
    Ok(())
}

/// Whether the project's planner process is alive.
pub fn is_live(runner: &dyn Runner, project: &Project) -> bool {
    runner
        .live_runners()
        .contains(&(planner::run_key(project.id), AgentRole::Planner))
}

/// A person types into the planner's terminal.
pub fn send_input(
    runner: &dyn Runner,
    actor: Actor,
    project: &Project,
    bytes: &[u8],
) -> AppResult<()> {
    person_only(actor, "write to the planner's terminal")?;
    if bytes.is_empty() {
        return Err(AppError::bad_request("nothing to write"));
    }
    runner
        .write(planner::run_key(project.id), AgentRole::Planner, bytes)
        .map_err(runner_error)
}

/// The planner's terminal follows the panel that shows it, within the bounds core sets.
pub fn resize(
    runner: &dyn Runner,
    actor: Actor,
    project: &Project,
    cols: u32,
    rows: u32,
) -> AppResult<()> {
    person_only(actor, "resize the planner's terminal")?;
    let size = toknila_core::instructions::terminal::clamp(cols, rows);
    runner
        .resize_role(planner::run_key(project.id), AgentRole::Planner, size)
        .map_err(runner_error)
}
