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

/// Where an accepted card's work lands.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    /// Merged into the base branch of the local repository. The default.
    #[default]
    Local,
    /// Pushed, opened as a pull request on GitHub, merged there when its checks pass.
    Github,
}

/// How GitHub merges a pull request (`gh pr merge --merge|--squash|--rebase`).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    /// A merge commit. The default.
    #[default]
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    /// The flag `gh pr merge` takes for it.
    pub fn flag(self) -> &'static str {
        match self {
            Self::Merge => "--merge",
            Self::Squash => "--squash",
            Self::Rebase => "--rebase",
        }
    }
}

/// A project's merge settings.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeSettings {
    pub mode: MergeMode,
    /// Title of a pull request; `{key}` and `{title}` are the card's.
    pub pr_title: String,
    pub method: MergeMethod,
}

impl MergeSettings {
    /// Pull request title of a project that does not name one.
    pub const DEFAULT_PR_TITLE: &str = "{key}: {title}";
}

impl Default for MergeSettings {
    fn default() -> Self {
        Self {
            mode: MergeMode::default(),
            pr_title: Self::DEFAULT_PR_TITLE.to_owned(),
            method: MergeMethod::default(),
        }
    }
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
    /// Where accepted work lands: the local base branch or a GitHub pull request.
    pub merge: MergeSettings,
    /// The command that checks the project's work (`cargo xtask verify`). Both agent roles are
    /// told to use it, and may run it without asking. `None`: the project names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_command: Option<String>,
}

impl Project {
    /// Work branch of a project that does not name one.
    pub const DEFAULT_WORK_BRANCH: &str = "develop";
    /// Release branch of a project that does not name one.
    pub const DEFAULT_RELEASE_BRANCH: &str = "main";

    /// The check command as a person typed it: trimmed, and `None` when empty (it clears the
    /// setting). It is one line without parentheses, because it goes into an agent's permission
    /// rule (`Bash(<command> *)`).
    pub fn clean_check_command(text: &str) -> Result<Option<String>, &'static str> {
        let text = text.trim();
        if text.contains(['\n', '\r', '(', ')']) {
            return Err("the check command is one line without parentheses");
        }
        Ok(Some(text.to_owned()).filter(|c| !c.is_empty()))
    }

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
    fn a_check_command_is_trimmed_cleared_when_empty_and_refused_when_unfit_for_a_rule() {
        assert_eq!(
            Project::clean_check_command("  cargo xtask verify "),
            Ok(Some("cargo xtask verify".to_owned()))
        );
        assert_eq!(Project::clean_check_command("   "), Ok(None));
        for bad in ["a\nb", "npm run (x)", "make) ; rm"] {
            assert!(Project::clean_check_command(bad).is_err(), "{bad:?}");
        }
    }

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
            merge: Default::default(),
            check_command: None,
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
