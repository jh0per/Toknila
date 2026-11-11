//! The hand-in a `review` item carries, and the text/design of the other kinds.

use super::*;
use crate::{CommentBody, CommentId, CommentKind, SinceReturn};
use hand_in::HandInStatus;

fn report(t: &Ticket, body: CommentBody) -> Comment {
    Comment {
        id: CommentId::new(),
        ticket_id: t.id,
        author: Actor::Agent,
        kind: CommentKind::Report,
        body,
        created_at: Timestamp(1),
        delivered_at: None,
    }
}

fn in_review() -> Ticket {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = Some(ReviewTurn::Person);
    t
}

fn with_comments(t: Ticket, comments: Vec<Comment>) -> Vec<InboxItem> {
    compute(&InboxData {
        tickets: vec![t],
        comments,
        ..InboxData::default()
    })
}

#[test]
fn a_review_item_carries_the_hand_in_without_another_request() {
    let t = in_review();
    let old = report(
        &t,
        CommentBody {
            gist: "First try".into(),
            how_to_verify: Some("- FAIL verify: red".into()),
            ..CommentBody::default()
        },
    );
    let new = report(
        &t,
        CommentBody {
            gist: "Inbox carries the hand-in".into(),
            what: Some("InboxItem got hand_in".into()),
            how_to_verify: Some(
                "- PASS verify: green\n- FAIL e2e: red\n- NOT_RUN web: no browser\nplain line"
                    .into(),
            ),
            consequences: Some("docs".into()),
            ..CommentBody::default()
        },
    );
    let got = with_comments(t, vec![old, new]);
    let h = got[0].hand_in.as_ref().expect("hand-in");
    assert_eq!(h.gist, "Inbox carries the hand-in");
    assert_eq!(h.what.as_deref(), Some("InboxItem got hand_in"));
    assert_eq!(h.left.as_deref(), Some("docs"));
    let seen: Vec<(Option<HandInStatus>, &str)> = h
        .checks
        .iter()
        .map(|c| (c.status, c.text.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            (Some(HandInStatus::Pass), "verify: green"),
            (Some(HandInStatus::Fail), "e2e: red"),
            (Some(HandInStatus::NotRun), "web: no browser"),
            (None, "plain line"),
        ]
    );
    assert_eq!(h.checks_line(), "PASS 1, FAIL 1, NOT_RUN 1");
}

#[test]
fn the_hand_in_carries_the_since_return_block() {
    let mut t = in_review();
    t.review.since_return = Some(SinceReturn::new(
        1,
        "aaa1111",
        "bbb2222",
        vec!["fix".into()],
        vec!["a.rs".into()],
    ));
    let c = report(
        &t,
        CommentBody {
            gist: "Fixed".into(),
            how_to_verify: Some("- PASS verify: ok".into()),
            ..CommentBody::default()
        },
    );
    let got = with_comments(t, vec![c]);
    let since = got[0]
        .hand_in
        .as_ref()
        .and_then(|h| h.since_return.as_ref());
    assert_eq!(
        since.map(|s| s.commits.clone()),
        Some(vec!["fix".to_owned()])
    );
}

#[test]
fn a_review_without_a_template_hand_in_has_none() {
    let t = in_review();
    let c = report(
        &t,
        CommentBody {
            gist: "Claude Code asks for permission: Bash".into(),
            ..CommentBody::default()
        },
    );
    assert_eq!(with_comments(t, vec![c])[0].hand_in, None);
}

#[test]
fn a_status_word_inside_a_longer_word_is_no_check() {
    assert_eq!(hand_in::parse_check("- PASSED it").status, None);
    assert_eq!(
        hand_in::parse_check("BLOCKED: no key").status,
        Some(HandInStatus::Blocked)
    );
}
