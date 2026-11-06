use serde::{Deserialize, Serialize};

use super::{Chain, Project, Ticket, TicketId, TicketKey};

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
    pub branch: String,
    /// Path-safe names under which the worktree lives: `<project>/<chain>/<slug>`.
    pub project: String,
    pub chain: String,
    pub slug: String,
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
        };
        let chain = Chain {
            id: ChainId::new(),
            project_id: project.id,
            name: "Auth Flow!".into(),
            goal: String::new(),
            autostart: true,
            review_mode: None,
            reviewer: None,
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
