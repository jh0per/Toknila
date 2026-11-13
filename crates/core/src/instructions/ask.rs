//! A person's question about a story, as the line the project's assistant takes in its terminal:
//! the question and the story's record (title, prompt, acceptance, checklist, hand-in, review
//! rounds with their verdicts, the last comments, the status and whose turn it is). One line, as
//! a terminal takes one. Pure.

use crate::inbox::hand_in;
use crate::wire::wire_name;
use crate::{Actor, Comment, CommentKind, ReviewTurn, Subtask, Ticket, TicketStatus};

/// How many of the newest comments the assistant is shown.
pub const COMMENTS_SHOWN: usize = 10;

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn who(a: Actor) -> &'static str {
    match a {
        Actor::Human => "person",
        Actor::Agent => "agent",
        Actor::System => "toknila",
        Actor::Assistant => "assistant",
    }
}

/// Whose turn the story is at, in words.
pub fn turn_words(t: &Ticket) -> String {
    let status = wire_name(&t.status).unwrap_or_default();
    if t.status != TicketStatus::Review {
        return status;
    }
    let turn = match t.review.turn {
        Some(ReviewTurn::Person) => "the person's turn: Accept or Return",
        Some(ReviewTurn::Agent { .. }) => "the reviewer agent's turn",
        Some(ReviewTurn::Check) => "the project's check is running",
        Some(ReviewTurn::Merging) => "merging",
        None => "no turn",
    };
    format!("{status}, {turn}")
}

/// The line the assistant is told for a question about `t`.
pub fn ask_line(t: &Ticket, subtasks: &[Subtask], comments: &[Comment], question: &str) -> String {
    let mut parts = vec![
        format!("Question on story {}: {}", t.key, flat(question)),
        "Record of the story follows; answer the question from it with `say` (about this story), in the language of the question, and change nothing".to_owned(),
        format!("Title: {}", flat(&t.title)),
        format!("Status: {}", turn_words(t)),
        format!("Prompt: {}", flat(&t.prompt)),
    ];
    let acceptance: Vec<String> = t
        .acceptance
        .iter()
        .enumerate()
        .map(|(i, a)| format!("{}. {}", i + 1, flat(a)))
        .collect();
    parts.push(format!("Acceptance: {}", or_none(&acceptance)));
    let tasks: Vec<String> = subtasks
        .iter()
        .map(|s| format!("[{}] {}", if s.done { "x" } else { " " }, flat(&s.text)))
        .collect();
    parts.push(format!("Checklist: {}", or_none(&tasks)));
    match hand_in::of_card(t, comments) {
        Some(h) => {
            let checks: Vec<String> = h
                .checks
                .iter()
                .map(|c| match c.status {
                    Some(s) => format!("{} {}", s.word(), flat(&c.text)),
                    None => flat(&c.text),
                })
                .collect();
            parts.push(format!(
                "Hand-in, What changed: {} {}",
                flat(&h.gist),
                flat(h.what.as_deref().unwrap_or(""))
            ));
            parts.push(format!("Hand-in, Checks: {}", or_none(&checks)));
            parts.push(format!(
                "Hand-in, Left: {}",
                flat(h.left.as_deref().unwrap_or("nothing was named"))
            ));
        }
        None => parts.push("Hand-in: none".to_owned()),
    }
    let rounds: Vec<String> = t
        .review
        .rounds
        .iter()
        .map(|r| {
            format!(
                "Round {} by {}: {:?}: {}",
                r.number,
                who(r.reviewer),
                r.verdict,
                flat(&r.comment)
            )
        })
        .collect();
    parts.push(format!("Review rounds: {}", or_none(&rounds)));
    let shown = comments.len().saturating_sub(COMMENTS_SHOWN);
    let last: Vec<String> = comments[shown..]
        .iter()
        .filter(|c| c.kind != CommentKind::Question)
        .map(|c| format!("{}: {}", who(c.author), flat(&c.body.gist)))
        .collect();
    parts.push(format!("Last comments: {}", or_none(&last)));
    parts.join(" | ")
}

fn or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommentBody, CommentId, TicketDraft, TicketId, TicketKey, Timestamp, Verdict};

    fn story() -> Ticket {
        let key = TicketKey::new("DEMO".parse().expect("project key"), 7).expect("ticket key");
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            crate::ChainId::new(),
            key,
            TicketDraft {
                title: "Login screen".to_owned(),
                prompt: "Build the login.\nKeep it small.".to_owned(),
                acceptance: vec![
                    "Errors are shown".to_owned(),
                    "Looks right on a phone".to_owned(),
                ],
                ..TicketDraft::default()
            },
            Timestamp(1),
        );
        t.status = TicketStatus::Review;
        t.review.turn = Some(ReviewTurn::Person);
        t.review.record(
            Actor::Agent,
            Verdict::RequestChanges,
            "PERSON 2. Looks right on a phone - open it on a phone [read]",
            Timestamp(5),
        );
        t
    }

    fn comment(
        t: &Ticket,
        author: Actor,
        kind: CommentKind,
        gist: &str,
        check: Option<&str>,
    ) -> Comment {
        Comment {
            id: CommentId::new(),
            ticket_id: t.id,
            author,
            kind,
            body: CommentBody {
                gist: gist.to_owned(),
                how_to_verify: check.map(str::to_owned),
                consequences: check.map(|_| "Nothing left".to_owned()),
                ..CommentBody::default()
            },
            created_at: Timestamp(2),
            delivered_at: None,
        }
    }

    #[test]
    fn the_line_holds_the_question_and_the_whole_record_on_one_line() {
        let t = story();
        let comments: Vec<Comment> = (0..12)
            .map(|i| {
                comment(
                    &t,
                    Actor::Human,
                    CommentKind::Instruction,
                    &format!("note {i}"),
                    None,
                )
            })
            .chain([comment(
                &t,
                Actor::Agent,
                CommentKind::Report,
                "Built the login",
                Some("- PASS cargo test: ok"),
            )])
            .collect();
        let tasks = vec![Subtask {
            id: crate::SubtaskId::new(),
            ticket_id: t.id,
            position: 1,
            text: "Write the form".to_owned(),
            done: true,
            done_by: Some(Actor::Agent),
        }];
        let line = ask_line(&t, &tasks, &comments, "шо треба зробити?");
        assert!(!line.contains('\n'), "{line}");
        for want in [
            "Question on story DEMO-7: шо треба зробити?",
            "Title: Login screen",
            "Status: review, the person's turn: Accept or Return",
            "Prompt: Build the login. Keep it small.",
            "Acceptance: 1. Errors are shown; 2. Looks right on a phone",
            "Checklist: [x] Write the form",
            "Hand-in, What changed: Built the login",
            "Hand-in, Checks: PASS cargo test: ok",
            "Hand-in, Left: Nothing left",
            "Round 1 by agent: RequestChanges: PERSON 2. Looks right on a phone - open it on a phone [read]",
        ] {
            assert!(line.contains(want), "missing {want:?} in {line}");
        }
        // The last ten comments only: of 13, notes 0, 1 and 2 are older than the ten shown.
        assert!(!line.contains("note 2;"), "{line}");
        assert!(
            line.contains("note 3;") && line.contains("agent: Built the login"),
            "{line}"
        );
    }
}
