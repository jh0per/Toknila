use serde::{Deserialize, Serialize};

use super::{ProjectId, ProjectKey};

/// Who reviews a finished ticket before it merges.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    /// A person reviews the diff. The default.
    #[default]
    Human,
    /// A reviewer agent reviews the diff.
    Agent,
    /// The reviewer agent first, then a person sees the diff with its remarks.
    Both,
}

/// One product: one monorepo, its chains, depot and settings.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub key: ProjectKey,
    pub name: String,
    /// Path to the repository on the host; empty until a repo is attached.
    pub repo_path: Option<String>,
    /// Branch ships start from and merge into (`develop`).
    pub work_branch: String,
    /// Branch that only receives tagged releases (`main`).
    pub release_branch: String,
    pub default_profile: String,
    pub default_review_mode: ReviewMode,
    /// Reviewer agent profile for tickets that name none; `None` derives it from the worker's
    /// profile (`<worker>-reviewer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reviewer: Option<String>,
    /// Most agents of this project that work at once (`schedule`); a card that starts beyond it
    /// is queued.
    pub max_parallel: u32,
}

impl Project {
    /// Work branch of a project that does not name one.
    pub const DEFAULT_WORK_BRANCH: &str = "develop";
    /// Release branch of a project that does not name one.
    pub const DEFAULT_RELEASE_BRANCH: &str = "main";

    /// Name of the agent profile `ticket` runs with: its own, else the project's default.
    pub fn profile_for<'a>(&'a self, ticket: &'a super::Ticket) -> &'a str {
        ticket.profile.as_deref().unwrap_or(&self.default_profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentProfile, ChainId, Ticket, TicketDraft, TicketId, Timestamp};

    #[test]
    fn a_ticket_profile_overrides_the_project_default() {
        let project = Project {
            id: ProjectId::new(),
            key: "DEMO".parse().expect("key"),
            name: "Demo".into(),
            repo_path: None,
            work_branch: "develop".into(),
            release_branch: "main".into(),
            default_profile: AgentProfile::SIM.into(),
            default_review_mode: ReviewMode::Human,
            default_reviewer: None,
            max_parallel: crate::schedule::DEFAULT_LIMIT,
        };
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            "DEMO-1".parse().expect("key"),
            TicketDraft::default(),
            Timestamp(1),
        );
        assert_eq!(project.profile_for(&t), "sim");
        t.profile = Some("claude-code".into());
        assert_eq!(project.profile_for(&t), "claude-code");
    }
}
