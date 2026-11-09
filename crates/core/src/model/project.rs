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
    /// Seconds a pull request may have no check reported before a person is asked what to do.
    #[serde(default = "MergeSettings::default_no_checks_timeout_secs")]
    pub no_checks_timeout_secs: u64,
}

impl MergeSettings {
    fn default_no_checks_timeout_secs() -> u64 {
        Self::DEFAULT_NO_CHECKS_TIMEOUT_SECS
    }

    /// Pull request title of a project that does not name one.
    pub const DEFAULT_PR_TITLE: &str = "{key}: {title}";
    /// Ten minutes.
    pub const DEFAULT_NO_CHECKS_TIMEOUT_SECS: u64 = 600;
    /// The longest timeout: one week.
    pub const MAX_NO_CHECKS_TIMEOUT_SECS: u64 = 7 * 24 * 3600;

    /// `secs` as a timeout, or why it is not one.
    pub fn check_no_checks_timeout(secs: u64) -> Result<u64, String> {
        if (1..=Self::MAX_NO_CHECKS_TIMEOUT_SECS).contains(&secs) {
            Ok(secs)
        } else {
            Err(format!(
                "the no-checks timeout must be 1 to {} seconds",
                Self::MAX_NO_CHECKS_TIMEOUT_SECS
            ))
        }
    }
}

impl Default for MergeSettings {
    fn default() -> Self {
        Self {
            mode: MergeMode::default(),
            pr_title: Self::DEFAULT_PR_TITLE.to_owned(),
            method: MergeMethod::default(),
            no_checks_timeout_secs: Self::DEFAULT_NO_CHECKS_TIMEOUT_SECS,
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
    /// How many times the daemon's loop (the check, then the reviewer agent) may send a card
    /// back to its worker before the card waits for a person.
    #[serde(default = "Project::default_review_rounds")]
    pub max_review_rounds: u32,
    /// Where accepted work lands: the local base branch or a GitHub pull request.
    pub merge: MergeSettings,
    /// The command that checks the project's work (`cargo xtask verify`). Both agent roles are
    /// told to use it, and may run it without asking. `None`: the project names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_command: Option<String>,
    /// The Claude model of the project's card agents (`--model`: an alias such as `sonnet`, or a
    /// full model name). `None`: the person's own Claude Code default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_model: Option<String>,
    /// The Claude model of the project's reviewer agents; `None`: the person's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_model: Option<String>,
    /// The Claude model of the project's planner; `None`: the person's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_model: Option<String>,
    /// When a card's agent wants to run a command.
    #[serde(default)]
    pub command_policy: super::CommandPolicy,
    /// Rules a person remembered with "Allow always" (`Bash(cargo test *)`): they go into every
    /// later run's allow list, and a prompt one covers is answered without asking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_commands: Vec<String>,
    /// Domains the project's reviewers may read on the web (`docs.rs`); empty: none, so a page a
    /// card cites stays unread.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewer_web_domains: Vec<String>,
    /// Hidden from the sidebar; its data is kept and it can be brought back.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
}

impl Project {
    /// Work branch of a project that does not name one.
    pub const DEFAULT_WORK_BRANCH: &str = "develop";
    /// Release branch of a project that does not name one.
    pub const DEFAULT_RELEASE_BRANCH: &str = "main";
    /// Rounds of the loop a project allows when it names no limit.
    pub const DEFAULT_REVIEW_ROUNDS: u32 = 3;
    /// The most rounds a project may allow.
    pub const MAX_REVIEW_ROUNDS: u32 = 20;

    /// [`Self::DEFAULT_REVIEW_ROUNDS`], for serde.
    pub fn default_review_rounds() -> u32 {
        Self::DEFAULT_REVIEW_ROUNDS
    }

    /// Checks a round limit a person typed (0 sends nothing back: the card goes to the person).
    pub fn check_review_rounds(n: u32) -> Result<u32, &'static str> {
        if n > Self::MAX_REVIEW_ROUNDS {
            return Err("the round limit is 0 to 20");
        }
        Ok(n)
    }

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

    /// The model name as a person typed it: trimmed, `None` when empty (it clears the setting).
    /// One word without whitespace or control characters, since it is one argument of a command.
    pub fn clean_model(text: &str) -> Result<Option<String>, &'static str> {
        let text = text.trim();
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) || text.starts_with('-') {
            return Err("the model is one word, such as sonnet or a full model name");
        }
        Ok(Some(text.to_owned()).filter(|m| !m.is_empty()))
    }

    /// The model the project names for an agent of `role` (a release agent works like a card's).
    pub fn model_for(&self, role: super::AgentRole) -> Option<&str> {
        use super::AgentRole::*;
        match role {
            Worker | Release => self.worker_model.as_deref(),
            Reviewer => self.reviewer_model.as_deref(),
            Planner => self.planner_model.as_deref(),
        }
    }

    /// Whether a remembered rule covers the Bash call `tool` (the call's text, `Bash: <command>`).
    pub fn allows_tool(&self, tool: &str) -> bool {
        super::command_policy::bash_command(tool).is_some_and(|command| {
            self.allowed_commands
                .iter()
                .any(|rule| super::command_policy::rule_covers(rule, command))
        })
    }

    /// Remembers the rule for the Bash call `tool`. Returns the rule, or `None` when the call is
    /// not a Bash call or gives no rule; a rule already remembered is kept once.
    pub fn remember_tool(&mut self, tool: &str) -> Option<String> {
        let rule = super::command_policy::rule_for(super::command_policy::bash_command(tool)?)?;
        if !self.allowed_commands.contains(&rule) {
            self.allowed_commands.push(rule.clone());
        }
        Some(rule)
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
    fn a_model_is_trimmed_cleared_when_empty_and_refused_when_it_is_not_one_word() {
        assert_eq!(
            Project::clean_model(" opus[1m] "),
            Ok(Some("opus[1m]".to_owned()))
        );
        assert_eq!(Project::clean_model("  "), Ok(None));
        for bad in ["two words", "a\nb", "--effort"] {
            assert!(Project::clean_model(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_ticket_profile_overrides_the_project_default() {
        let mut project = Project {
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
            max_review_rounds: Project::DEFAULT_REVIEW_ROUNDS,
            merge: Default::default(),
            check_command: None,
            worker_model: None,
            reviewer_model: None,
            planner_model: None,
            command_policy: Default::default(),
            allowed_commands: Vec::new(),
            reviewer_web_domains: Vec::new(),
            archived: false,
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

        // Each role gets the model the project names for it; a release agent works like a card's.
        use crate::AgentRole::*;
        let all = [Worker, Reviewer, Planner, Release];
        assert_eq!(all.map(|r| project.model_for(r)), [None; 4]);
        project.worker_model = Some("sonnet".into());
        project.reviewer_model = Some("haiku".into());
        project.planner_model = Some("opus".into());
        assert_eq!(
            all.map(|r| project.model_for(r)),
            [Some("sonnet"), Some("haiku"), Some("opus"), Some("sonnet")]
        );
    }
}
