use super::*;
use crate::{ThreadId, ThreadKind, ThreadMessageId, ThreadState, TicketId, Timestamp, thread};

fn say(kind: MessageKind, by: Actor, text: &str, at: i64) -> ThreadMessage {
    ThreadMessage {
        id: ThreadMessageId::new(),
        thread_id: ThreadId::new(),
        author: by,
        kind,
        text: text.to_owned(),
        created_at: Timestamp(at),
    }
}

fn proposal() -> EditProposal {
    EditProposal::of(
        "old",
        &["a".to_owned()],
        Some("new".to_owned()),
        Some(vec!["a".to_owned(), "b".to_owned()]),
    )
}

#[test]
fn a_discussion_is_born_approved_so_it_never_blocks_and_is_not_a_design() {
    let th = thread::open(
        ThreadId::new(),
        TicketId::new(),
        1,
        ThreadKind::Discussion,
        TITLE,
        Actor::Human,
        Timestamp(5),
    )
    .expect("open");
    assert_eq!(th.state, ThreadState::Approved);
    assert_eq!(th.approved_version, None);
    assert!(thread::open_threads(std::slice::from_ref(&th), |_| true).is_empty());
    assert_eq!(
        thread::phase(std::slice::from_ref(&th)),
        crate::Phase::Implement
    );
}

#[test]
fn a_proposal_keeps_only_what_differs_and_applies_only_to_the_card_it_was_made_against() {
    let same = EditProposal::of("old", &[], Some("old".to_owned()), Some(vec![]));
    assert!(same.is_empty());
    let p = proposal();
    assert!(p.applies_to("old", &["a".to_owned()]));
    assert!(!p.applies_to("changed since", &["a".to_owned()]));
    assert!(!p.applies_to("old", &[]));
}

#[test]
fn a_proposal_is_pending_until_a_later_message_names_it() {
    let m = say(
        MessageKind::EditProposal,
        Actor::Agent,
        &serde_json::to_string(&proposal()).expect("json"),
        10,
    );
    assert_eq!(proposal_of(&m), Some(proposal()));
    assert_eq!(decision(std::slice::from_ref(&m), &m), Decision::Pending);
    let other = say(MessageKind::EditAccepted, Actor::Human, "not-its-id", 20);
    assert_eq!(
        decision(&[m.clone(), other], &m),
        Decision::Pending,
        "a decision about another message is not this one's"
    );
    let no = say(
        MessageKind::EditRejected,
        Actor::Human,
        &m.id.to_string(),
        20,
    );
    assert_eq!(decision(&[m.clone(), no], &m), Decision::Rejected);
    let yes = say(
        MessageKind::EditAccepted,
        Actor::Human,
        &m.id.to_string(),
        20,
    );
    assert_eq!(decision(&[m.clone(), yes], &m), Decision::Accepted);
}

#[test]
fn the_transcript_says_who_spoke_and_what_became_of_each_proposal() {
    let ask = say(MessageKind::Message, Actor::Human, "stricter\nerrors", 1);
    let reply = say(MessageKind::Message, Actor::Agent, "I would add 4xx", 2);
    let prop = say(
        MessageKind::EditProposal,
        Actor::Agent,
        &serde_json::to_string(&proposal()).expect("json"),
        3,
    );
    let yes = say(
        MessageKind::EditAccepted,
        Actor::Human,
        &prop.id.to_string(),
        4,
    );
    let all = [ask, reply, prop, yes];
    assert_eq!(
        transcript(&all),
        vec![
            "person: stricter errors".to_owned(),
            "assistant: I would add 4xx".to_owned(),
            "assistant proposed an edit (accepted: the card above has it); prompt to: new; \
             acceptance to: \"a\"; \"b\""
                .to_owned(),
        ]
    );
}
