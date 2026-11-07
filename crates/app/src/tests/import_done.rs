//! Importing a chain file whose cards are marked done: in order they walk the state machine,
//! behind an open card they are merged out of order and carry the mark.

use toknila_core::{TicketStatus, spec};

use super::Mem;
use crate::chains;

/// A is open; B (after A) and C (after B) are done; D is done and independent.
const FILE: &str = r#"
[chain]
name = "imported"
autostart = false

[[ticket]]
id = "A"
subtasks = ["one", "two"]
done_subtasks = [1]

[[ticket]]
id = "B"
done = true
after = ["A"]

[[ticket]]
id = "C"
done = true
after = ["B"]

[[ticket]]
id = "D"
done = true
"#;

fn imported() -> Mem {
    let mut mem = Mem::demo();
    chains::import(&mut mem, "DEMO", FILE, None).expect("imports");
    mem
}

fn card(mem: &Mem, title: &str) -> toknila_core::Ticket {
    let id = mem
        .view
        .tickets
        .iter()
        .find(|t| t.title == title)
        .expect("card")
        .id;
    mem.ticket_by_id(id)
}

#[test]
fn done_cards_behind_an_open_card_are_merged_with_the_mark() {
    let mem = imported();
    let [a, b, c, d] = ["A", "B", "C", "D"].map(|t| card(&mem, t));
    assert_eq!(a.status, TicketStatus::Ready);
    assert_eq!(d.status, TicketStatus::Merged);
    assert_eq!(d.done_out_of_order, None);
    for t in [&b, &c] {
        assert_eq!(t.status, TicketStatus::Merged, "{}", t.key);
        let behind = &t.done_out_of_order.as_ref().expect("marked").behind;
        assert_eq!(behind, std::slice::from_ref(&a.key));
    }
}

#[test]
fn the_journal_records_the_out_of_order_merge() {
    let mem = imported();
    let b = card(&mem, "B");
    let moved = mem.events.iter().any(|e| {
        matches!(e, toknila_core::events::DomainEvent::TicketStatusChanged { key, to, ticket, .. }
            if *key == b.key && *to == TicketStatus::Merged && ticket.done_out_of_order.is_some())
    });
    assert!(moved, "{:?}", mem.kinds());
}

#[test]
fn a_chain_imported_from_a_spec_folder_with_a_gap_has_no_card_left_behind() {
    let text = "## Phase 1: A\n- [x] T001 a\n\n## Phase 2: B\n- [ ] T002 b\n\n## Phase 3: C\n- [x] T003 c\n";
    let imp = spec::import(text, "gap", None, &[]).expect("spec");
    let mut mem = Mem::demo();
    chains::import(&mut mem, "DEMO", &imp.toml, None).expect("imports");
    let statuses: Vec<_> = ["A", "B", "C"]
        .iter()
        .map(|t| card(&mem, t).status)
        .collect();
    assert_eq!(
        statuses,
        [
            TicketStatus::Merged,
            TicketStatus::Ready,
            TicketStatus::Merged
        ]
    );
}
