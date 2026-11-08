//! The planner: a project's main agent. It plans (chains, cards, dependencies, threads) and
//! starts, pauses, resumes, cancels, accepts, returns, reopens and merges nothing; those stay
//! with the owner. Pure rules only; the application orders them into commands and the daemon
//! decides which routes a planner's token reaches.
//!
//! A planner run belongs to a project, not to a card, but the runner machinery is keyed by card
//! ids. A planner run is therefore keyed by [`run_key`], its project's id read as a card id; ids
//! are ULIDs, so the key cannot be a real card's, and [`project_of`] reads it back.

use crate::{Chain, Project, ProjectId, Ticket, TicketId};

/// The id a project's planner run is keyed by in the runner.
pub fn run_key(project: ProjectId) -> TicketId {
    TicketId(project.0)
}

/// The project a planner run key stands for.
pub fn project_of(run_key: TicketId) -> ProjectId {
    ProjectId(run_key.0)
}

/// Why a planner may not do what it asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// The thing belongs to another project than the planner's.
    #[error("{what} belongs to another project; this planner works on {project} only")]
    OtherProject { what: String, project: String },
    /// A change of the graph in a chain that starts its cards by itself could start one.
    #[error(
        "chain {chain} starts its ready cards by itself (autostart), so changing its cards' \
         order or dependencies could start work, and only the owner starts cards. Ask the owner \
         to turn autostart off, or plan in a new chain"
    )]
    AutostartChain { chain: String },
    /// The card's agent has its prompt already, or the card is done.
    #[error(
        "{card} is {status}; the planner edits only cards that have not started. Tell the owner \
         what should change, or open a thread on the card"
    )]
    CardStarted { card: String, status: String },
    /// A planner reads the repository, so the project needs one.
    #[error("project {0} has no repository for the planner to read")]
    NoRepository(String),
    /// An archived project runs no agents.
    #[error("project {0} is archived")]
    Archived(String),
}

/// Whether a thing of project `owner` is in the planner's project.
pub fn ensure_in_project(
    planner: ProjectId,
    owner: ProjectId,
    what: impl Into<String>,
    project_key: &str,
) -> Result<(), Refusal> {
    if planner == owner {
        Ok(())
    } else {
        Err(Refusal::OtherProject {
            what: what.into(),
            project: project_key.to_owned(),
        })
    }
}

/// Whether the planner may change the graph of `chain` (add a card, add or remove a dependency,
/// move a card). Only in a chain that waits for the owner to start its cards.
pub fn ensure_graph_change(chain: &Chain) -> Result<(), Refusal> {
    if chain.autostart {
        Err(Refusal::AutostartChain {
            chain: chain.name.clone(),
        })
    } else {
        Ok(())
    }
}

/// Whether the planner may rewrite `card`: only before it starts.
pub fn ensure_card_editable(card: &Ticket) -> Result<(), Refusal> {
    if card.status.can_move() {
        Ok(())
    } else {
        Err(Refusal::CardStarted {
            card: card.key.to_string(),
            status: crate::wire::wire_name(&card.status).unwrap_or_default(),
        })
    }
}

/// Autostart of a chain a planner creates: off, the owner starts the cards.
pub const NEW_CHAIN_AUTOSTART: bool = false;

/// Whether a planner may run for `project`.
pub fn ensure_can_run(project: &Project) -> Result<(), Refusal> {
    if project.archived {
        return Err(Refusal::Archived(project.key.to_string()));
    }
    if project
        .repo_path
        .as_deref()
        .is_none_or(|p| p.trim().is_empty())
    {
        return Err(Refusal::NoRepository(project.key.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(autostart: bool) -> Chain {
        Chain {
            id: crate::ChainId::new(),
            project_id: ProjectId::new(),
            name: "plan".to_owned(),
            goal: String::new(),
            autostart,
            review_mode: None,
            reviewer: None,
            source: None,
        }
    }

    #[test]
    fn a_project_id_round_trips_through_the_run_key() {
        let p = ProjectId::new();
        assert_eq!(project_of(run_key(p)), p);
        assert_eq!(run_key(p).to_string(), p.to_string());
    }

    #[test]
    fn the_graph_of_an_autostart_chain_is_off_limits_and_a_manual_chain_is_open() {
        assert!(ensure_graph_change(&chain(false)).is_ok());
        let refused = ensure_graph_change(&chain(true)).expect_err("refused");
        assert!(matches!(refused, Refusal::AutostartChain { .. }));
        assert!(refused.to_string().contains("only the owner starts cards"));
    }

    #[test]
    fn a_thing_of_another_project_is_refused() {
        let (a, b) = (ProjectId::new(), ProjectId::new());
        assert!(ensure_in_project(a, a, "chain x", "A").is_ok());
        let e = ensure_in_project(a, b, "chain x", "B").expect_err("other project");
        assert!(e.to_string().contains("another project"), "{e}");
    }

    #[test]
    fn a_planner_needs_a_repository_and_a_project_that_is_not_archived() {
        let mut p = Project {
            id: ProjectId::new(),
            key: "DEMO".parse().expect("key"),
            name: "Demo".to_owned(),
            repo_path: None,
            work_branch: "develop".to_owned(),
            release_branch: "main".to_owned(),
            default_profile: "sim".to_owned(),
            default_review_mode: crate::ReviewMode::Human,
            default_reviewer: None,
            max_parallel: 2,
            merge: crate::MergeSettings::default(),
            check_command: None,
            worker_model: None,
            reviewer_model: None,
            planner_model: None,
            command_policy: Default::default(),
            allowed_commands: Vec::new(),
            archived: false,
        };
        assert!(matches!(ensure_can_run(&p), Err(Refusal::NoRepository(_))));
        p.repo_path = Some("/work/demo".to_owned());
        assert!(ensure_can_run(&p).is_ok());
        p.archived = true;
        assert!(matches!(ensure_can_run(&p), Err(Refusal::Archived(_))));
    }
}
