//! Projects: the top of the hierarchy, addressed by key.

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::{
    AgentProfile, CommandPolicy, MergeMethod, MergeMode, MergeSettings, Project, ProjectId,
    ReviewMode, schedule,
};

use crate::{AppError, AppResult, Reader, UnitOfWork, resolve};

/// `POST /v1/projects`: a new project; unset fields take the project defaults.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewProject {
    /// Short uppercase key, prefix of ticket keys (`DEMO`).
    pub key: String,
    pub name: Option<String>,
    pub repo_path: Option<String>,
    pub work_branch: Option<String>,
    pub release_branch: Option<String>,
    pub default_profile: Option<String>,
    pub default_review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; unset means `<worker profile>-reviewer`.
    pub default_reviewer: Option<String>,
    /// Most agents working at once; default 2.
    pub max_parallel: Option<u32>,
    /// How many times the loop (check, reviewer) may return a card to its worker; default 3.
    pub max_review_rounds: Option<u32>,
    /// The command that checks the work; an empty text names none.
    pub check_command: Option<String>,
    /// Where accepted work lands; default `local`.
    pub merge_mode: Option<MergeMode>,
    /// When the agent of a card wants to run a command; default `auto`.
    pub command_policy: Option<CommandPolicy>,
}

/// `PATCH /v1/projects/{project}`: the settings to change; unset fields stay.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PatchProject {
    /// Most agents working at once (1 to 64).
    pub max_parallel: Option<u32>,
    /// How many times the loop (check, reviewer) may return a card to its worker before the
    /// card waits for a person (0 to 20).
    pub max_review_rounds: Option<u32>,
    /// Who reviews finished cards of the project unless a chain or ticket says.
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; an empty text goes back to `<worker profile>-reviewer`.
    pub reviewer: Option<String>,
    /// Profile of cards that name none; a name from `GET /v1/profiles`.
    pub default_profile: Option<String>,
    /// The base branch ships start from and merges go into.
    pub work_branch: Option<String>,
    /// `local` merges into the base branch; `github` opens a pull request and merges it there.
    pub merge_mode: Option<MergeMode>,
    /// Title of the pull requests; `{key}` and `{title}` are the card's.
    pub pr_title: Option<String>,
    /// How a pull request merges: `merge`, `squash` or `rebase`.
    pub merge_method: Option<MergeMethod>,
    /// Seconds a pull request may report no check before a person is asked (default 600).
    pub no_checks_timeout_secs: Option<u64>,
    /// The command that checks the work (`cargo xtask verify`); agents are told to run it and may
    /// without asking. An empty text clears it.
    pub check_command: Option<String>,
    /// The command a release runs to bump the version (`npm version {version}
    /// --no-git-tag-version`; `{version}` is the version). An empty text clears it.
    pub bump_command: Option<String>,
    /// The Claude model of the project's card agents (`sonnet`, `opus`, a full model name). An
    /// empty text clears it: the person's own Claude Code default.
    pub worker_model: Option<String>,
    /// The Claude model of the project's reviewers; an empty text clears it.
    pub reviewer_model: Option<String>,
    /// The Claude model of the project's planner; an empty text clears it.
    pub planner_model: Option<String>,
    /// When the agent of a card wants to run a command: `ask` (the person answers each prompt),
    /// `auto` (Claude Code's auto mode decides) or `trust` (never asked).
    pub command_policy: Option<CommandPolicy>,
    /// The commands remembered with "Allow always" (`Bash(cargo test *)`); the list replaces the
    /// remembered ones, so a rule is removed by sending the list without it.
    pub allowed_commands: Option<Vec<String>>,
    /// The domains the project's reviewers may read on the web (`docs.rs`); the list replaces the
    /// old one, an empty list denies the web again.
    pub reviewer_web_domains: Option<Vec<String>>,
}

/// Creates a project; its key must be valid and unused.
pub fn create<U: UnitOfWork + ?Sized>(uow: &mut U, draft: NewProject) -> AppResult<Project> {
    let key = draft
        .key
        .parse()
        .map_err(|e: toknila_core::KeyError| AppError::bad_request(e.to_string()))?;
    if draft.merge_mode == Some(MergeMode::Github)
        && draft
            .repo_path
            .as_deref()
            .is_none_or(|r| r.trim().is_empty())
    {
        return Err(AppError::bad_request(
            "pull-request mode needs the project's repository (a project without one has no ships to push)",
        ));
    }
    let p = Project {
        id: ProjectId::new(),
        name: draft.name.unwrap_or_else(|| draft.key.clone()),
        key,
        repo_path: draft.repo_path,
        work_branch: draft
            .work_branch
            .unwrap_or_else(|| Project::DEFAULT_WORK_BRANCH.to_owned()),
        release_branch: draft
            .release_branch
            .unwrap_or_else(|| Project::DEFAULT_RELEASE_BRANCH.to_owned()),
        default_profile: draft
            .default_profile
            .unwrap_or_else(|| AgentProfile::SIM.to_owned()),
        // The loop first (check, reviewer), then the person sees a card that already passes.
        default_review_mode: draft.default_review_mode.unwrap_or(ReviewMode::Both),
        max_review_rounds: draft
            .max_review_rounds
            .map_or(
                Ok(Project::DEFAULT_REVIEW_ROUNDS),
                Project::check_review_rounds,
            )
            .map_err(AppError::bad_request)?,
        default_reviewer: draft.default_reviewer.filter(|r| !r.trim().is_empty()),
        max_parallel: draft
            .max_parallel
            .map_or(Ok(schedule::DEFAULT_LIMIT), schedule::check_limit)
            .map_err(|e| AppError::bad_request(e.to_string()))?,
        merge: toknila_core::MergeSettings {
            mode: draft.merge_mode.unwrap_or_default(),
            ..Default::default()
        },
        check_command: match draft.check_command {
            Some(c) => Project::clean_check_command(&c).map_err(AppError::bad_request)?,
            None => None,
        },
        bump_command: None,
        worker_model: None,
        reviewer_model: None,
        planner_model: None,
        command_policy: draft.command_policy.unwrap_or(CommandPolicy::NEW_PROJECT),
        allowed_commands: Vec::new(),
        reviewer_web_domains: Vec::new(),
        archived: false,
    };
    uow.insert_project(&p)?;
    uow.emit(&DomainEvent::ProjectCreated { project: p.clone() })?;
    Ok(p)
}

/// Changes a project's settings. A higher parallel limit starts waiting cards at once; a lower
/// one never stops a running agent, it only holds back the next start.
pub fn patch<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &str,
    patch: PatchProject,
) -> AppResult<Project> {
    let mut p = resolve::project(uow, project)?;
    if let Some(n) = patch.max_parallel {
        p.max_parallel =
            schedule::check_limit(n).map_err(|e| AppError::bad_request(e.to_string()))?;
    }
    if let Some(n) = patch.max_review_rounds {
        p.max_review_rounds = Project::check_review_rounds(n).map_err(AppError::bad_request)?;
    }
    let name = |what: &str, v: String| match v.trim() {
        "" => Err(AppError::bad_request(format!("{what} cannot be blank"))),
        v => Ok(v.to_owned()),
    };
    if let Some(v) = patch.default_profile {
        p.default_profile = name("the default profile", v)?;
    }
    if let Some(v) = patch.work_branch {
        p.work_branch = name("the base branch", v)?;
    }
    if let Some(v) = patch.pr_title {
        p.merge.pr_title = name("the pull request title", v)?;
    }
    if let Some(m) = patch.merge_method {
        p.merge.method = m;
    }
    if let Some(secs) = patch.no_checks_timeout_secs {
        p.merge.no_checks_timeout_secs =
            MergeSettings::check_no_checks_timeout(secs).map_err(AppError::bad_request)?;
    }
    if let Some(c) = patch.check_command {
        p.check_command = Project::clean_check_command(&c).map_err(AppError::bad_request)?;
    }
    if let Some(c) = patch.bump_command {
        p.bump_command = Project::clean_bump_command(&c).map_err(AppError::bad_request)?;
    }
    for (field, text) in [
        (&mut p.worker_model, patch.worker_model),
        (&mut p.reviewer_model, patch.reviewer_model),
        (&mut p.planner_model, patch.planner_model),
    ] {
        if let Some(m) = text {
            *field = Project::clean_model(&m).map_err(AppError::bad_request)?;
        }
    }
    if let Some(c) = patch.command_policy {
        p.command_policy = c;
    }
    if let Some(rules) = patch.allowed_commands {
        p.allowed_commands =
            toknila_core::command_policy::clean_rules(rules).map_err(AppError::bad_request)?;
    }
    if let Some(domains) = patch.reviewer_web_domains {
        p.reviewer_web_domains =
            toknila_core::command_policy::clean_domains(domains).map_err(AppError::bad_request)?;
    }
    if let Some(m) = patch.merge_mode {
        if m == MergeMode::Github && p.repo_path.as_deref().is_none_or(|r| r.trim().is_empty()) {
            return Err(AppError::bad_request(
                "pull-request mode needs the project's repository (a project without one has no ships to push)",
            ));
        }
        p.merge.mode = m;
    }
    if let Some(m) = patch.review_mode {
        p.default_review_mode = m;
    }
    if let Some(r) = patch.reviewer {
        p.default_reviewer = Some(r.trim().to_owned()).filter(|r| !r.is_empty());
    }
    uow.update_project(&p)?;
    uow.emit(&DomainEvent::ProjectUpdated { project: p.clone() })?;
    crate::schedule::run(uow, p.id)?;
    Ok(p)
}

/// The project `t` belongs to (through its chain).
pub fn of_ticket<R: Reader + ?Sized>(r: &R, t: &toknila_core::Ticket) -> AppResult<Project> {
    let chain = r
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain of the ticket"))?;
    r.project(chain.project_id)?
        .ok_or_else(|| AppError::not_found("project of the chain"))
}

/// "Allow always": remembers in the project of `t` the rule for the Bash call `tool` (its text,
/// `Bash: <command>`), so later runs allow it and a running agent's prompt it covers is answered
/// without asking. Returns the rule; `None` (nothing changes) for a call that gives none.
pub fn remember_command<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &toknila_core::Ticket,
    tool: &str,
) -> AppResult<Option<String>> {
    let mut p = of_ticket(uow, t)?;
    let known = p.allowed_commands.len();
    let Some(rule) = p.remember_tool(tool) else {
        return Ok(None);
    };
    if p.allowed_commands.len() != known {
        uow.update_project(&p)?;
        uow.emit(&DomainEvent::ProjectUpdated { project: p })?;
    }
    Ok(Some(rule))
}

/// Hides a project from the sidebar, or brings it back. Nothing is deleted. Archiving is refused
/// while any agent of the project works (a card holds a slot or a runner is alive).
pub fn set_archived<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: &str,
    archived: bool,
) -> AppResult<Project> {
    let mut p = resolve::project(uow, project)?;
    if p.archived == archived {
        return Ok(p);
    }
    if archived {
        ensure_idle(uow, &p, "archiving")?;
    }
    p.archived = archived;
    uow.update_project(&p)?;
    uow.emit(&DomainEvent::ProjectUpdated { project: p.clone() })?;
    Ok(p)
}

/// Refuses `doing` (archiving, deleting) the project while any of its agents works: a card holds
/// a slot or a runner is alive. The person is told to stop them first.
pub(crate) fn ensure_idle<R: Reader + ?Sized>(r: &R, p: &Project, doing: &str) -> AppResult<()> {
    let used = crate::schedule::plan_of(r, p)?.used;
    if used > 0 {
        return Err(AppError::conflict(format!(
            "{} has {used} agent(s) working; stop them before {doing} the project",
            p.key
        )));
    }
    Ok(())
}
