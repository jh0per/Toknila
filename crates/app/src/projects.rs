//! Projects: the top of the hierarchy, addressed by key.

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::{
    AgentProfile, MergeMethod, MergeMode, Project, ProjectId, ReviewMode, schedule,
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
}

/// `PATCH /v1/projects/{project}`: the settings to change; unset fields stay.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PatchProject {
    /// Most agents working at once (1 to 64).
    pub max_parallel: Option<u32>,
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
    /// The command that checks the work (`cargo xtask verify`); agents are told to run it and may
    /// without asking. An empty text clears it.
    pub check_command: Option<String>,
}

/// Creates a project; its key must be valid and unused.
pub fn create<U: UnitOfWork + ?Sized>(uow: &mut U, draft: NewProject) -> AppResult<Project> {
    let key = draft
        .key
        .parse()
        .map_err(|e: toknila_core::KeyError| AppError::bad_request(e.to_string()))?;
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
        default_review_mode: draft.default_review_mode.unwrap_or_default(),
        default_reviewer: draft.default_reviewer.filter(|r| !r.trim().is_empty()),
        max_parallel: draft
            .max_parallel
            .map_or(Ok(schedule::DEFAULT_LIMIT), schedule::check_limit)
            .map_err(|e| AppError::bad_request(e.to_string()))?,
        merge: Default::default(),
        check_command: None,
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
    if let Some(c) = patch.check_command {
        p.check_command = Project::clean_check_command(&c).map_err(AppError::bad_request)?;
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
