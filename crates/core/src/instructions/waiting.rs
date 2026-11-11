//! What a run reads first beyond the task: the instructions that no run has seen yet, and the
//! Return the card came back with. Pure.

use crate::{Actor, Comment, CommentId, CommentKind, Review, template};

/// The Return a restarted worker answers: the last review round, when it asked for changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Returned {
    pub round: u32,
    pub by: Option<Actor>,
    /// The commit that was handed in and returned; `None` when it was not recorded.
    pub commit: Option<String>,
    pub text: String,
    /// What changed since the Return, once the hand-in that answers it is made.
    pub since: Option<crate::SinceReturn>,
}

impl Returned {
    /// The Return the card is back with, if it is.
    pub fn of(review: &Review) -> Option<Self> {
        review.open_return().map(|r| Self {
            round: r.number,
            by: Some(r.reviewer),
            commit: review.handed_in.clone(),
            text: r.comment.trim().to_owned(),
            since: review.since_return.clone(),
        })
    }
}

/// The reviewer's kickoff sentence about a hand-in that answers a Return: read what changed since
/// it first. Empty when the hand-in answers none.
pub(super) fn reviewer_part(returned: Option<&Returned>) -> String {
    let Some(s) = returned.and_then(|r| r.since.as_ref()) else {
        return String::new();
    };
    let (from, to) = (short_commit(&s.from), short_commit(&s.to));
    format!(
        " This hand-in answers a Return (round {n}): read the \"Since the Return\" block of your \
         instructions first (`git log --oneline {from}..{to}`, `git diff --stat {from}..{to}`), \
         then the rest.",
        n = s.round
    )
}

/// Who returned a round, as the agent reads it.
pub fn returned_by(who: Actor) -> &'static str {
    match who {
        Actor::Human => "the person",
        Actor::Agent => "the reviewer",
        Actor::System => "the project's check",
    }
}

/// A commit id as people read it: the first seven characters.
pub fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// The instructions no run has seen yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Waiting {
    /// Every pending instruction comment: the run that reads them marks them delivered.
    pub ids: Vec<CommentId>,
    /// What the run is shown, oldest first, in the comment template. A comment that only
    /// repeats the Return's text is left out: the Return is shown on its own.
    pub texts: Vec<String>,
}

/// The pending instruction comments of a card (a person's context given before the start, a
/// Return's text, the reviewer's remarks), oldest first.
pub fn waiting(comments: &[Comment], returned: Option<&Returned>) -> Waiting {
    let mut pending: Vec<&Comment> = comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction && c.delivered_at.is_none())
        .collect();
    pending.sort_by_key(|c| c.created_at);
    let mut out = Waiting::default();
    for c in pending {
        out.ids.push(c.id);
        let text = template::render(c.kind, &c.body).trim_end().to_owned();
        let repeats = returned.is_some_and(|r| !r.text.is_empty() && text.contains(&r.text));
        if !repeats {
            out.texts.push(text);
        }
    }
    out
}

/// The kickoff sentences about what waits for the run.
pub(super) fn kickoff_part(waiting: &[String], returned: Option<&Returned>) -> String {
    let mut out = String::new();
    if let Some(r) = returned {
        let by = r.by.map_or("a reviewer", returned_by);
        let what = match &r.commit {
            Some(c) => format!("your hand-in {}", short_commit(c)),
            None => "your hand-in".to_owned(),
        };
        out.push_str(&format!(
            " This card was returned: round {n}, by {by}, sent back {what}. The Return comes \
             first, before anything else: \"{text}\". Make that change, commit it, then call \
             `finished` naming the new commit; a hand-in with the same commit is refused.",
            n = r.round,
            text = one_line(&r.text),
        ));
    }
    if !waiting.is_empty() {
        out.push_str(
            " Instructions are waiting for you in your Toknila instructions (\"Instructions \
             waiting for you\"): read them first, they come before the task.",
        );
    }
    out
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The sections of the instructions file for the Return and the waiting instructions.
pub(super) fn sections(waiting: &[String], returned: Option<&Returned>) -> String {
    let mut out = String::new();
    if let Some(r) = returned {
        let by = r.by.map_or("a reviewer", returned_by);
        out.push_str(&format!(
            "\n## This card was returned\n\nRound {n} was returned by {by}",
            n = r.round
        ));
        if let Some(c) = &r.commit {
            out.push_str(&format!(
                " after your hand-in at commit {}",
                short_commit(c)
            ));
        }
        out.push_str(
            ". Do this first: read the Return below, make the change it asks for, commit it, \
             then call `finished` naming the new commit. A hand-in at the same commit is \
             refused.\n\n",
        );
        if r.text.is_empty() {
            out.push_str("(The Return has no text.)\n");
        }
        for line in r.text.lines() {
            out.push_str(&super::line_quoted(line));
        }
    }
    if !waiting.is_empty() {
        out.push_str(
            "\n## Instructions waiting for you\n\n\
             These were given to this card and no agent has acted on them yet. They come before \
             the task above: read them first and do what they ask. Oldest first:\n",
        );
        for text in waiting {
            out.push('\n');
            for line in text.lines() {
                out.push_str(&super::line_quoted(line));
            }
        }
    }
    out
}
