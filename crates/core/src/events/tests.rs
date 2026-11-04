use serde_json::{Value, json};

use super::*;
use crate::{ChainId, ReleaseChannel, ReleaseId, StartMode, TicketDraft, TicketId, Timestamp};

fn ticket(status: TicketStatus) -> Ticket {
    let mut t = Ticket::new_backlog(
        "01JBQ7ZJ0000000000000000T1"
            .parse::<TicketId>()
            .expect("id"),
        "01JBQ7ZJ0000000000000000C1".parse::<ChainId>().expect("id"),
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

fn payload(e: &DomainEvent) -> Value {
    serde_json::to_value(e).expect("serialize")
}

#[test]
fn status_change_carries_key_from_to_reason_and_ticket() {
    let before = ticket(TicketStatus::Ready);
    let after = ticket(TicketStatus::Running);
    let e = DomainEvent::ticket_changed(&before, &after);
    assert_eq!(
        (e.entity(), e.kind(), e.entity_id().as_str()),
        ("ticket", "status_changed", "01JBQ7ZJ0000000000000000T1")
    );
    assert_eq!(
        payload(&e),
        json!({
            "key": "DEMO-1",
            "from": "ready",
            "to": "running",
            "reason": null,
            "ticket": serde_json::to_value(&after).expect("ticket"),
        })
    );
}

#[test]
fn same_status_is_an_update() {
    let before = ticket(TicketStatus::Ready);
    let mut after = before.clone();
    after.title = "B".into();
    let e = DomainEvent::ticket_changed(&before, &after);
    assert_eq!((e.entity(), e.kind()), ("ticket", "updated"));
    assert_eq!(
        payload(&e),
        json!({ "ticket": serde_json::to_value(&after).expect("t") })
    );
}

#[test]
fn release_change_names_version_and_states() {
    let before = Release::planned(
        "01JBQ7ZJ0000000000000000R1"
            .parse::<ReleaseId>()
            .expect("id"),
        "01JBQ7ZJ0000000000000000C1".parse::<ChainId>().expect("id"),
        "1.0".into(),
        ReleaseChannel::TagOnly,
        StartMode::Manual,
    );
    let mut after = before.clone();
    after.state = ReleaseState::Cut;
    let e = DomainEvent::release_changed(&before, &after);
    assert_eq!((e.entity(), e.kind()), ("release", "state_changed"));
    let p = payload(&e);
    assert_eq!(
        (&p["version"], &p["from"], &p["to"]),
        (&json!("1.0"), &json!("planned"), &json!("cut"))
    );
}

#[test]
fn edge_events_belong_to_their_chain() {
    let chain_id: ChainId = "01JBQ7ZJ0000000000000000C1".parse().expect("id");
    let edge = Edge {
        chain_id,
        from: crate::NodeRef::Ticket("01JBQ7ZJ0000000000000000T1".parse().expect("id")),
        to: crate::NodeRef::Ticket("01JBQ7ZJ0000000000000000T2".parse().expect("id")),
        kind: crate::EdgeKind::Normal,
    };
    let e = DomainEvent::EdgeAdded { edge };
    assert_eq!(
        (e.entity(), e.kind(), e.entity_id()),
        ("chain", "edge_added", chain_id.to_string())
    );
    assert_eq!(payload(&e)["edge"]["kind"], json!("normal"));
}
