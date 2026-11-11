use super::*;
use crate::{ReleaseChannel, ReleaseId, ReleaseState, StartMode, TicketDraft, TicketId};

fn card(status: TicketStatus) -> Ticket {
    let mut t = Ticket::new_backlog(
        TicketId::new(),
        ChainId::new(),
        "DEMO-1".parse().expect("key"),
        TicketDraft {
            title: "A".into(),
            ..TicketDraft::default()
        },
        Timestamp(5),
    );
    t.status = status;
    t
}

#[test]
fn an_action_applies_to_a_card_in_the_status_that_allows_it() {
    use SuggestionAction::*;
    let ok = |a, s| check("next", Some(a), Some(&card(s)));
    assert_eq!(ok(Start, TicketStatus::Ready), Ok(()));
    assert_eq!(ok(Answer, TicketStatus::Waiting), Ok(()));
    assert_eq!(ok(Accept, TicketStatus::Review), Ok(()));
    assert_eq!(ok(Release, TicketStatus::Merged), Ok(()));
    let refused = ok(Accept, TicketStatus::Running).expect_err("running is not accepted");
    assert_eq!(
        refused.to_string(),
        "DEMO-1 is running: accept does not apply to it now"
    );
    assert!(ok(Start, TicketStatus::Running).is_err());
    assert!(ok(Answer, TicketStatus::Running).is_err());
}

#[test]
fn a_suggestion_has_a_text_and_an_action_goes_with_its_card() {
    assert_eq!(check(" ", None, None), Err(SuggestionRefusal::NoText));
    assert_eq!(check("Nothing waits for you", None, None), Ok(()));
    assert_eq!(
        check("x", Some(SuggestionAction::Start), None),
        Err(SuggestionRefusal::ActionNeedsCard)
    );
    assert_eq!(
        check("x", None, Some(&card(TicketStatus::Ready))),
        Err(SuggestionRefusal::CardNeedsAction)
    );
}

#[test]
fn a_status_change_ends_the_suggestions_about_that_card_and_a_release_those_of_its_chain() {
    let before = card(TicketStatus::Ready);
    let mut after = before.clone();
    after.status = TicketStatus::Running;
    let moved = DomainEvent::ticket_changed(&before, &after);
    assert_eq!(
        ends(&moved),
        Some(Ends::Card("DEMO-1".parse().expect("key")))
    );
    // An edit that leaves the status alone ends nothing.
    assert_eq!(ends(&DomainEvent::ticket_changed(&before, &before)), None);
    let chain = ChainId::new();
    let release = crate::Release {
        id: ReleaseId::new(),
        chain_id: chain,
        version: "1.0.0".into(),
        channel: ReleaseChannel::TagOnly,
        start_mode: StartMode::Manual,
        state: ReleaseState::Planned,
        branch: None,
        cut_at: None,
        released_at: None,
        tag: None,
    };
    assert_eq!(
        ends(&DomainEvent::ReleaseCreated { release }),
        Some(Ends::Release(chain))
    );
}

#[test]
fn the_wire_shape_is_pinned() {
    let s = Suggestion {
        project: "DEMO".parse().expect("key"),
        text: "Start DEMO-2".into(),
        card: Some("DEMO-2".parse().expect("key")),
        chain: None,
        action: Some(SuggestionAction::Start),
        at: Timestamp(7),
    };
    assert_eq!(
        serde_json::to_string(&s).expect("json"),
        r#"{"project":"DEMO","text":"Start DEMO-2","card":"DEMO-2","action":"start","at":7}"#
    );
}
