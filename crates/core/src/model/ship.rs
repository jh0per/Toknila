use serde::{Deserialize, Serialize};

use super::{BaseBehind, Chain, MergeMode, Project, StalePolicy, Ticket, TicketId, TicketKey};
use crate::pr::PrSpec;

/// The worktree and branch a ticket works in, once the shipyard has made them.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ship {
    pub ticket_id: TicketId,
    pub key: TicketKey,
    /// `ship/<chain>/<slug>`.
    pub branch: String,
    /// Absolute path of the worktree on the host.
    pub path: String,
}

/// What a shipyard needs to make a ship: decided here, executed by an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipSpec {
    pub key: TicketKey,
    /// The project's repository on the host.
    pub repo: String,
    /// Branch the ship starts from (the project's work branch).
    pub base: String,
    /// The ship starts from the remote's `base` (pull-request mode: the base moves on GitHub),
    /// fetched first, not from the local branch.
    pub remote: bool,
    /// What a person said about a local base branch that is behind its upstream.
    pub stale: StalePolicy,
    pub branch: String,
    /// Path-safe names under which the worktree lives: `<project>/<chain>/<slug>`.
    pub project: String,
    pub chain: String,
    pub slug: String,
}

/// Where a project's planner reads the code: decided here, executed by the shipyard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerSpec {
    /// The project's repository on the host.
    pub repo: String,
    /// The project's work branch.
    pub base: String,
    /// Pull-request mode: the base moves on the remote, so the planner reads the fetched
    /// `origin/<base>` in a checkout of its own, not the working copy.
    pub remote: bool,
    /// Path-safe project name, the folder under the ships root.
    pub project: String,
}

impl PlannerSpec {
    /// The planner of `project`; `None` when it has no repository.
    pub fn for_project(project: &Project) -> Option<Self> {
        let repo = project
            .repo_path
            .as_deref()
            .filter(|r| !r.trim().is_empty())?;
        Some(Self {
            repo: repo.to_owned(),
            base: project.work_branch.clone(),
            remote: project.merge.mode == MergeMode::Github,
            project: slugify(&project.key.to_string()),
        })
    }
}

/// Where the planner runs and what the host found about the code it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerSite {
    /// The folder the planner starts in.
    pub dir: String,
    /// Local mode: the base branch is behind its upstream, so the code may be out of date.
    pub behind: Option<BaseBehind>,
}

/// What a merge needs: the ship's branch goes into the base branch of the repository with a
/// merge commit (two parents), so the history shows the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSpec {
    pub key: TicketKey,
    pub repo: String,
    /// The branch that receives the merge.
    pub base: String,
    /// The ship's branch.
    pub branch: String,
    /// The merge commit's message.
    pub message: String,
    /// Set in pull-request mode: the branch goes through a pull request instead of a local merge.
    pub pr: Option<PrSpec>,
    /// Where the ship's work is looked at before it is pushed or merged (`guard`).
    pub look: ShipLook,
}

/// Where to look at a ship's work: its worktree, and what the branch is counted against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipLook {
    /// The worktree's path on the host.
    pub path: String,
    /// The commit-ish the ship started from (`ShipSpec::start_point`): commits beyond it are the
    /// ship's own.
    pub start: String,
}

/// Longest slug part, so paths stay short on Windows.
const MAX_SLUG: usize = 40;

/// Lower-case ASCII letters and digits joined by single dashes; never empty.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(MAX_SLUG);
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "x".to_owned()
    } else {
        out.to_owned()
    }
}

impl ShipSpec {
    /// The merge of this ship's branch into its base for a ticket called `title`.
    pub fn merge_spec(&self, title: &str, ship_path: &str) -> MergeSpec {
        MergeSpec {
            key: self.key.clone(),
            repo: self.repo.clone(),
            base: self.base.clone(),
            branch: self.branch.clone(),
            message: format!("Merge {}: {} {}", self.branch, self.key, title),
            pr: None,
            look: ShipLook {
                path: ship_path.to_owned(),
                start: self.start_point(),
            },
        }
    }

    /// What the ship's worktree is made from: the base branch, or the remote's copy of it.
    pub fn start_point(&self) -> String {
        if self.remote {
            format!("origin/{}", self.base)
        } else {
            self.base.clone()
        }
    }

    /// The ship of `ticket`; `None` when the project has no repository, so the ticket runs
    /// without one.
    pub fn for_ticket(project: &Project, chain: &Chain, ticket: &Ticket) -> Option<Self> {
        let repo = project
            .repo_path
            .as_deref()
            .filter(|r| !r.trim().is_empty())?;
        let chain = slugify(&chain.name);
        let slug = slugify(&format!("{} {}", ticket.key, ticket.title));
        Some(Self {
            key: ticket.key.clone(),
            repo: repo.to_owned(),
            base: project.work_branch.clone(),
            remote: project.merge.mode == MergeMode::Github,
            stale: StalePolicy::default(),
            branch: format!("ship/{chain}/{slug}"),
            project: slugify(&project.key.to_string()),
            chain,
            slug,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentProfile, ChainId, ProjectId, ReviewMode, TicketDraft, Timestamp};

    fn fixture(repo: Option<&str>) -> (Project, Chain, Ticket) {
        let project = Project {
            id: ProjectId::new(),
            key: "DEMO".parse().expect("key"),
            name: "Demo".into(),
            repo_path: repo.map(str::to_owned),
            work_branch: "develop".into(),
            release_branch: "main".into(),
            default_profile: AgentProfile::SIM.into(),
            default_review_mode: ReviewMode::Human,
            default_reviewer: None,
            max_parallel: 2,
            max_review_rounds: 3,
            merge: Default::default(),
            check_command: None,
            setup_command: None,
            bump_command: None,
            worker_model: None,
            reviewer_model: None,
            planner_model: None,
            command_policy: Default::default(),
            allowed_commands: Vec::new(),
            reviewer_web_domains: Vec::new(),
            archived: false,
            simulated: false,
        };
        let chain = Chain {
            id: ChainId::new(),
            project_id: project.id,
            name: "Auth Flow!".into(),
            goal: String::new(),
            autostart: true,
            review_mode: None,
            reviewer: None,
            source: None,
            position: 0,
            version: None,
        };
        let draft = TicketDraft {
            title: "Add login: OAuth".into(),
            ..TicketDraft::default()
        };
        let ticket = Ticket::new_backlog(
            TicketId::new(),
            chain.id,
            "DEMO-3".parse().expect("key"),
            draft,
            Timestamp(1),
        );
        (project, chain, ticket)
    }

    #[test]
    fn spec_names_branch_and_path_parts_from_chain_and_ticket() {
        let (p, c, t) = fixture(Some("/repo"));
        let s = ShipSpec::for_ticket(&p, &c, &t).expect("spec");
        assert_eq!(s.branch, "ship/auth-flow/demo-3-add-login-oauth");
        assert_eq!(
            (s.project.as_str(), s.chain.as_str(), s.slug.as_str()),
            ("demo", "auth-flow", "demo-3-add-login-oauth")
        );
        assert_eq!((s.repo.as_str(), s.base.as_str()), ("/repo", "develop"));
    }

    #[test]
    fn the_merge_names_the_branch_the_card_and_the_base() {
        let (p, c, t) = fixture(Some("/repo"));
        let spec = ShipSpec::for_ticket(&p, &c, &t).expect("spec");
        let merge = spec.merge_spec("Add login: OAuth", "/ships/x");
        assert_eq!(
            (merge.repo.as_str(), merge.base.as_str()),
            ("/repo", "develop")
        );
        assert_eq!(merge.branch, "ship/auth-flow/demo-3-add-login-oauth");
        assert_eq!(
            merge.message,
            "Merge ship/auth-flow/demo-3-add-login-oauth: DEMO-3 Add login: OAuth"
        );
    }

    #[test]
    fn pull_request_mode_starts_ships_from_the_remote_base() {
        let (mut p, c, t) = fixture(Some("/repo"));
        let local = ShipSpec::for_ticket(&p, &c, &t).expect("spec");
        assert_eq!(
            (local.remote, local.start_point().as_str()),
            (false, "develop")
        );
        p.merge.mode = MergeMode::Github;
        let remote = ShipSpec::for_ticket(&p, &c, &t).expect("spec");
        assert_eq!(
            (remote.remote, remote.start_point().as_str()),
            (true, "origin/develop")
        );
        assert_eq!(
            remote.merge_spec("x", "/ships/x").base,
            "develop",
            "the pull request goes into the plain base"
        );
    }

    #[test]
    fn no_repository_means_no_ship() {
        let (p, c, t) = fixture(None);
        assert_eq!(ShipSpec::for_ticket(&p, &c, &t), None);
        let (p, c, t) = fixture(Some("  "));
        assert_eq!(ShipSpec::for_ticket(&p, &c, &t), None);
    }

    #[test]
    fn slugs_are_path_safe_short_and_never_empty() {
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("Ünï!!"), "n");
        assert_eq!(slugify("???"), "x");
        assert!(slugify(&"ab ".repeat(40)).len() <= 40);
        assert!(!slugify(&"ab ".repeat(40)).ends_with('-'));
    }
}
