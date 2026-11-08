//! Review of a finished ticket: who reviews (the mode), where the card stands, and the rounds
//! already held. The rules that move a card through review are in [`crate::machine`].

use serde::{Deserialize, Serialize};

use super::{Actor, Chain, Project, ReviewMode, Ticket, Timestamp};

/// A reviewer's decision about the work.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The work is good as it is.
    Approve,
    /// The work needs another pass; the comment says what to change.
    RequestChanges,
}

/// One verdict on a ticket. Numbers count from 1 over every verdict, a person's and the
/// reviewer agent's, in the order they were given.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRound {
    pub number: u32,
    /// A person (`human`) or the reviewer agent (`agent`).
    pub reviewer: Actor,
    pub verdict: Verdict,
    /// The remarks; empty when the reviewer had none.
    pub comment: String,
    pub decided_at: Timestamp,
}

/// Whose verdict a card in Review waits for.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum ReviewTurn {
    /// The reviewer agent is looking at it. With `decides` its verdict moves the card (agent
    /// mode); without, the card then waits for a person (both mode).
    Agent { decides: bool },
    /// A person.
    Person,
    /// The work was accepted and its merge into the base branch is running on the host. Nobody
    /// is awaited; the outcome arrives as an event (`MergeDone`, or a block with the reason).
    Merging,
}

/// Why a card waits for a person in Review beyond the ordinary hand-in; shown in the inbox.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "note", rename_all = "snake_case")]
pub enum ReviewNote {
    /// The reviewer ended its turn (or its process) without a verdict, even after one nudge.
    /// `lines` are the last lines of its screen, ANSI removed (at most 15; may be empty).
    NoVerdict { lines: Vec<String> },
}

/// A ticket's review state: whose turn it is while in Review, and every round so far.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    /// `None` outside Review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<ReviewTurn>,
    /// Oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rounds: Vec<ReviewRound>,
    /// The reviewer was already nudged for a verdict in this round of review.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nudged: bool,
    /// Why the card is for a person, when the reviewer gave no verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<ReviewNote>,
}

impl Review {
    /// True before the first review: nothing to show or store.
    pub fn is_empty(&self) -> bool {
        self.turn.is_none() && self.rounds.is_empty() && !self.nudged && self.note.is_none()
    }

    /// Adds a round by `reviewer`; its number follows the last one.
    pub fn record(&mut self, reviewer: Actor, verdict: Verdict, comment: &str, at: Timestamp) {
        let number = u32::try_from(self.rounds.len() + 1).unwrap_or(u32::MAX);
        self.rounds.push(ReviewRound {
            number,
            reviewer,
            verdict,
            comment: comment.trim().to_owned(),
            decided_at: at,
        });
    }
}

impl ReviewMode {
    /// Whom a card that has just been handed in waits for.
    pub fn first_turn(self) -> ReviewTurn {
        match self {
            Self::Human => ReviewTurn::Person,
            Self::Agent => ReviewTurn::Agent { decides: true },
            Self::Both => ReviewTurn::Agent { decides: false },
        }
    }
}

/// The review mode of `ticket`: its own, else its chain's, else the project's default.
pub fn resolve_mode(ticket: &Ticket, chain: &Chain, project: &Project) -> ReviewMode {
    ticket
        .review_mode
        .or(chain.review_mode)
        .unwrap_or(project.default_review_mode)
}

/// Name of the reviewer profile that goes with a worker profile: `sim` is reviewed by
/// `sim-reviewer`, `claude-code` by `claude-code-reviewer`, any `x` by `x-reviewer`.
pub fn reviewer_profile(worker: &str) -> String {
    format!("{worker}-reviewer")
}

/// The reviewer profile of `ticket`: its own setting, else its chain's, else the project's,
/// else the one named after the profile the ticket's worker runs with.
pub fn resolve_reviewer(ticket: &Ticket, chain: &Chain, project: &Project) -> String {
    [
        ticket.reviewer.as_deref(),
        chain.reviewer.as_deref(),
        project.default_reviewer.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|name| !name.trim().is_empty())
    .map_or_else(
        || reviewer_profile(project.profile_for(ticket)),
        str::to_owned,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, ProjectId, TicketDraft, TicketId};

    fn setup() -> (Ticket, Chain, Project) {
        let project = Project {
            id: ProjectId::new(),
            key: "DEMO".parse().expect("key"),
            name: "Demo".into(),
            repo_path: None,
            work_branch: "develop".into(),
            release_branch: "main".into(),
            default_profile: "sim".into(),
            default_review_mode: ReviewMode::Human,
            default_reviewer: None,
            max_parallel: 2,
            merge: Default::default(),
            check_command: None,
            worker_model: None,
            reviewer_model: None,
            planner_model: None,
            archived: false,
        };
        let chain = Chain {
            id: ChainId::new(),
            project_id: project.id,
            name: "c".into(),
            goal: String::new(),
            autostart: true,
            review_mode: None,
            reviewer: None,
            source: None,
        };
        let ticket = Ticket::new_backlog(
            TicketId::new(),
            chain.id,
            "DEMO-1".parse().expect("key"),
            TicketDraft::default(),
            Timestamp(1),
        );
        (ticket, chain, project)
    }

    #[test]
    fn the_mode_defaults_to_human() {
        let (t, c, p) = setup();
        assert_eq!(resolve_mode(&t, &c, &p), ReviewMode::Human);
    }

    #[test]
    fn the_project_default_applies_when_nothing_is_set_below() {
        let (t, c, mut p) = setup();
        p.default_review_mode = ReviewMode::Both;
        assert_eq!(resolve_mode(&t, &c, &p), ReviewMode::Both);
    }

    #[test]
    fn the_chain_overrides_the_project() {
        let (t, mut c, mut p) = setup();
        p.default_review_mode = ReviewMode::Both;
        c.review_mode = Some(ReviewMode::Agent);
        assert_eq!(resolve_mode(&t, &c, &p), ReviewMode::Agent);
    }

    #[test]
    fn the_ticket_overrides_the_chain_and_the_project() {
        let (mut t, mut c, mut p) = setup();
        p.default_review_mode = ReviewMode::Both;
        c.review_mode = Some(ReviewMode::Agent);
        t.review_mode = Some(ReviewMode::Human);
        assert_eq!(resolve_mode(&t, &c, &p), ReviewMode::Human);
    }

    #[test]
    fn the_reviewer_profile_resolves_ticket_then_chain_then_project_then_the_worker_name() {
        let (mut t, mut c, mut p) = setup();
        assert_eq!(resolve_reviewer(&t, &c, &p), "sim-reviewer");
        t.profile = Some("claude-code".into());
        assert_eq!(resolve_reviewer(&t, &c, &p), "claude-code-reviewer");
        p.default_reviewer = Some("from-project".into());
        assert_eq!(resolve_reviewer(&t, &c, &p), "from-project");
        c.reviewer = Some("from-chain".into());
        assert_eq!(resolve_reviewer(&t, &c, &p), "from-chain");
        t.reviewer = Some("from-ticket".into());
        assert_eq!(resolve_reviewer(&t, &c, &p), "from-ticket");
        t.reviewer = Some("  ".into());
        assert_eq!(resolve_reviewer(&t, &c, &p), "from-chain", "blank is unset");
    }

    #[test]
    fn rounds_are_numbered_from_one_in_order() {
        let mut r = Review::default();
        assert!(r.is_empty());
        r.record(
            Actor::Agent,
            Verdict::RequestChanges,
            " tighten it ",
            Timestamp(2),
        );
        r.record(Actor::Human, Verdict::Approve, "", Timestamp(3));
        let numbers: Vec<u32> = r.rounds.iter().map(|x| x.number).collect();
        assert_eq!(numbers, [1, 2]);
        assert_eq!(r.rounds[0].comment, "tighten it");
        assert!(!r.is_empty());
    }

    #[test]
    fn the_reviewer_profile_is_named_after_the_worker_profile() {
        assert_eq!(reviewer_profile("sim"), "sim-reviewer");
        assert_eq!(reviewer_profile("claude-code"), "claude-code-reviewer");
    }
}
