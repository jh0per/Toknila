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

/// How one acceptance item stands, in the reviewer's eyes.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemOutcome {
    /// The item holds.
    Pass,
    /// The item does not hold.
    Fail,
}

/// How the reviewer came to its judgement of an item.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemBasis {
    /// The reviewer ran the check or test that shows it and quoted the result.
    Ran,
    /// The reviewer judged it by reading the diff and the files.
    #[default]
    Read,
    /// The check or test the item names could not be run.
    NotRun,
}

impl ItemBasis {
    /// The word the verdict text and the Review card show.
    pub fn word(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::Read => "read",
            Self::NotRun => "NOT_RUN",
        }
    }

    /// The basis a verdict line ends with (`... - evidence [ran]`), if it ends with one.
    pub fn of_line(line: &str) -> Option<(Self, &str)> {
        let line = line.trim_end();
        [Self::Ran, Self::Read, Self::NotRun]
            .into_iter()
            .find_map(|b| {
                let tail = format!(" [{}]", b.word());
                line.strip_suffix(tail.as_str()).map(|rest| (b, rest))
            })
    }
}

/// The reviewer's judgement of one acceptance item.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemVerdict {
    /// The item's 1-based position in the ticket's acceptance list.
    pub item: u32,
    pub outcome: ItemOutcome,
    /// Whether the reviewer ran what shows the item or only read; `read` when it says nothing.
    #[serde(default)]
    pub basis: ItemBasis,
    /// One line of evidence.
    #[serde(default)]
    pub note: String,
}

/// Why the reviewer's item verdicts are not a judgement of the ticket's acceptance list.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ItemsError {
    #[error(
        "the verdict must judge every acceptance item: {0} is missing (give `items` with pass or fail for each)"
    )]
    Missing(u32),
    #[error("acceptance item {0} does not exist, or is judged twice")]
    Unknown(u32),
    #[error("an approval cannot have a failing acceptance item ({0})")]
    ApprovedWithFailure(u32),
}

/// Checks that `items` judge each of the `acceptance` items exactly once, and that an approval
/// has none failing. A ticket without acceptance items takes no item verdicts.
pub fn check_items(
    acceptance: &[String],
    items: &[ItemVerdict],
    verdict: Verdict,
) -> Result<(), ItemsError> {
    let count = u32::try_from(acceptance.len()).unwrap_or(u32::MAX);
    let mut seen = std::collections::HashSet::new();
    for i in items {
        if i.item == 0 || i.item > count || !seen.insert(i.item) {
            return Err(ItemsError::Unknown(i.item));
        }
    }
    if let Some(missing) = (1..=count).find(|n| !seen.contains(n)) {
        return Err(ItemsError::Missing(missing));
    }
    if verdict == Verdict::Approve
        && let Some(f) = items.iter().find(|i| i.outcome == ItemOutcome::Fail)
    {
        return Err(ItemsError::ApprovedWithFailure(f.item));
    }
    Ok(())
}

/// The verdict as the round and the worker read it: one `PASS`/`FAIL` line per acceptance item
/// in the list's order, then the reviewer's own remarks.
pub fn render_items(acceptance: &[String], items: &[ItemVerdict], comment: &str) -> String {
    let mut sorted: Vec<&ItemVerdict> = items.iter().collect();
    sorted.sort_by_key(|i| i.item);
    let mut out = String::new();
    for i in sorted {
        let text = acceptance
            .get((i.item as usize).saturating_sub(1))
            .map_or("", String::as_str);
        let word = match i.outcome {
            ItemOutcome::Pass => "PASS",
            // A failure that rests on nothing run is not a failure: it is recorded as not run.
            ItemOutcome::Fail if i.basis == ItemBasis::NotRun => "NOT_RUN",
            ItemOutcome::Fail => "FAIL",
        };
        out.push_str(&format!("{word} {}. {text}", i.item));
        if !i.note.trim().is_empty() {
            out.push_str(&format!(" - {}", i.note.trim()));
        }
        out.push_str(&format!(" [{}]\n", i.basis.word()));
    }
    out.push_str(comment.trim());
    out.trim().to_owned()
}

/// Whether a request for changes rests on nothing: no item fails on a basis of `ran` or `read`
/// (every item is `not_run`, or passes), and, when there are no items at all, the comment says
/// that nothing was run (`NOT_RUN` or "not run"). Such a verdict is for a person, not a round.
pub fn request_is_unfounded(items: &[ItemVerdict], comment: &str) -> bool {
    if items.is_empty() {
        let c = comment.to_lowercase();
        return c.contains("not_run") || c.contains("not run");
    }
    !items
        .iter()
        .any(|i| i.outcome == ItemOutcome::Fail && i.basis != ItemBasis::NotRun)
}

/// One verdict on a ticket. Numbers count from 1 over every verdict, a person's and the
/// reviewer agent's, in the order they were given.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRound {
    pub number: u32,
    /// A person (`human`), the reviewer agent (`agent`) or the daemon's check (`system`).
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
    /// The daemon is running the project's check command on the ship. Nobody is awaited; the
    /// result arrives as an event (`CheckDone`).
    Check,
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
    /// Pull-request mode: the host reported no check on the pull request for the project's
    /// timeout. A person decides: merge now, keep waiting, or block.
    NoChecks,
    /// The check command could not start on this machine (its program was not found), so the
    /// agent's work was never judged. A person decides: accept, return, or fix the machine.
    CheckCannotRun { command: String },
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
    /// The commit the ship's branch was at when the worker last handed in; `None` before the
    /// first hand-in, or when the host could not look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handed_in: Option<String>,
    /// The merge commits the daemon made itself to bring the ship up to its base since the
    /// hand-in; they are not work the agent did not hand in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synced: Vec<String>,
    /// What changed since the Return, for a hand-in that answers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_return: Option<super::SinceReturn>,
}

impl Review {
    /// True before the first review: nothing to show or store.
    pub fn is_empty(&self) -> bool {
        self.turn.is_none()
            && self.rounds.is_empty()
            && !self.nudged
            && self.note.is_none()
            && self.handed_in.is_none()
            && self.synced.is_empty()
            && self.since_return.is_none()
    }

    /// How many times the loop (the check or the reviewer agent) has sent the card back since a
    /// person last decided. A person's decision starts the count again.
    pub fn auto_returns(&self) -> u32 {
        let since = self
            .rounds
            .iter()
            .rposition(|r| r.reviewer == Actor::Human)
            .map_or(0, |i| i + 1);
        let n = self.rounds[since..]
            .iter()
            .filter(|r| r.verdict == Verdict::RequestChanges)
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// The last round when it asked for changes and no later one has decided: the Return the
    /// worker is working on, if the card is back with it.
    pub fn open_return(&self) -> Option<&ReviewRound> {
        self.rounds
            .last()
            .filter(|r| r.verdict == Verdict::RequestChanges)
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

    #[test]
    fn each_item_line_ends_with_how_the_reviewer_came_to_it_and_reads_back() {
        let acceptance = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        let item = |n, basis, note: &str| ItemVerdict {
            item: n,
            outcome: ItemOutcome::Pass,
            basis,
            note: note.to_owned(),
        };
        let text = render_items(
            &acceptance,
            &[
                item(1, ItemBasis::Ran, "test result: ok. 3 passed"),
                item(2, ItemBasis::Read, ""),
                item(3, ItemBasis::NotRun, "no cargo"),
            ],
            "ok",
        );
        assert_eq!(
            text,
            "PASS 1. a - test result: ok. 3 passed [ran]\nPASS 2. b [read]\nPASS 3. c - no cargo [NOT_RUN]\nok"
        );
        let basis: Vec<_> = text
            .lines()
            .map(|l| ItemBasis::of_line(l).map(|(b, _)| b))
            .collect();
        assert_eq!(
            basis,
            [
                Some(ItemBasis::Ran),
                Some(ItemBasis::Read),
                Some(ItemBasis::NotRun),
                None
            ]
        );
        let v: ItemVerdict = serde_json::from_str(r#"{"item":1,"outcome":"pass"}"#).expect("json");
        assert_eq!(v.basis, ItemBasis::Read, "an item without a basis was read");
    }

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
            max_review_rounds: 3,
            merge: Default::default(),
            check_command: None,
            bump_command: None,
            worker_model: None,
            reviewer_model: None,
            planner_model: None,
            command_policy: Default::default(),
            allowed_commands: Vec::new(),
            reviewer_web_domains: Vec::new(),
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
            version: None,
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
