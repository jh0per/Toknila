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
        "epic {chain} starts its ready stories by itself (autostart), so changing its stories' \
         order or dependencies could start work, and only the owner starts stories. Ask the owner \
         to turn autostart off, or plan in a new epic"
    )]
    AutostartChain { chain: String },
    /// The card's agent has its prompt already, or the card is done.
    #[error(
        "{card} is {status}; the planner edits only stories that have not started. Tell the owner \
         what should change, or open a thread on the story"
    )]
    CardStarted { card: String, status: String },
    /// On the roadmap the planner changes the top level; stories go only to an epic it made in
    /// this turn or one the person named.
    #[error(
        "the person is on the roadmap, where you change epics; stories of epic {chain} are not \
         yours to change now. Add stories only to the epic you made in this turn or one the \
         person named, or ask them to open epic {chain}"
    )]
    RoadmapStory { chain: String },
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

/// Whether the planner may change the stories of `chain` in the turn `turn`: the conversation
/// from the person's latest message on, oldest first. Only a turn that began with a message
/// about the roadmap limits it, to an epic Toknila reported as added in the turn or one the
/// message names (its name or title).
pub fn ensure_story_target(
    turn: &[crate::conversation::Message],
    chain: &Chain,
) -> Result<(), Refusal> {
    use crate::conversation::{About, Author};
    let Some(first) = turn.first() else {
        return Ok(());
    };
    if first.author != Author::Person || first.about != Some(About::Roadmap) {
        return Ok(());
    }
    let said = first.text.to_lowercase();
    let named = [chain.name.clone(), chain.display_title()]
        .iter()
        .any(|n| !n.trim().is_empty() && said.contains(&n.trim().to_lowercase()));
    let made = turn.iter().skip(1).any(|m| {
        m.author == Author::Toknila
            && m.about == Some(About::chain_of(chain))
            && m.text.starts_with("added epic ")
    });
    if named || made {
        Ok(())
    } else {
        Err(Refusal::RoadmapStory {
            chain: chain.name.clone(),
        })
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
            title: None,
            goal: String::new(),
            autostart,
            review_mode: None,
            reviewer: None,
            source: None,
            position: 0,
            version: None,
        }
    }

    fn said(
        author: crate::conversation::Author,
        about: Option<crate::conversation::About>,
        text: &str,
    ) -> crate::conversation::Message {
        crate::conversation::Message {
            id: "x".to_owned(),
            author,
            text: text.to_owned(),
            about,
            attachments: vec![],
            thread: None,
            at: crate::Timestamp(1),
        }
    }

    #[test]
    fn on_the_roadmap_stories_go_to_an_epic_made_in_the_turn_or_named_and_no_other() {
        use crate::conversation::{About, Author};
        let c = chain(false);
        let roadmap = said(Author::Person, Some(About::Roadmap), "add hiding a project");
        let made = said(
            Author::Toknila,
            Some(About::chain_of(&c)),
            "added epic Plan v0.4.0",
        );
        let named = said(Author::Person, Some(About::Roadmap), "put it in plan");
        let other = said(Author::Person, Some(About::chain_of(&c)), "here");
        assert!(ensure_story_target(&[], &c).is_ok());
        assert!(ensure_story_target(&[roadmap.clone(), made], &c).is_ok());
        assert!(ensure_story_target(&[named], &c).is_ok());
        assert!(ensure_story_target(&[other], &c).is_ok());
        let e = ensure_story_target(&[roadmap], &c).expect_err("another epic");
        assert!(matches!(e, Refusal::RoadmapStory { .. }), "{e}");
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
        assert!(
            refused
                .to_string()
                .contains("only the owner starts stories")
        );
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
            max_review_rounds: crate::Project::DEFAULT_REVIEW_ROUNDS,
            merge: crate::MergeSettings::default(),
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
        assert!(matches!(ensure_can_run(&p), Err(Refusal::NoRepository(_))));
        p.repo_path = Some("/work/demo".to_owned());
        assert!(ensure_can_run(&p).is_ok());
        p.archived = true;
        assert!(matches!(ensure_can_run(&p), Err(Refusal::Archived(_))));
    }
}
