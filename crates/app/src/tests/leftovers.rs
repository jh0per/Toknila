//! The inbox over the ports, and the three leftover actions on an imported spec folder.

use toknila_core::inbox::{InboxKind, leftovers::LeftoverError};
use toknila_core::spec::{self, ExportCard, ExportSubtask};
use toknila_core::{CommentKind, NodeRef, Ticket, TicketStatus};

use super::Mem;
use crate::leftovers::{LeftoverAction, resolve};
use crate::{ErrorKind, chains, inbox};

/// Phase 1 has two open tasks; phase 2 is done, so it is imported behind an open card.
const TASKS: &str = "## Phase 1: Setup\n- [x] T001 init\n- [ ] T002 wire it\n- [ ] T003 test it\n\n## Phase 2: Build\n- [x] T004 build\n";

fn imported() -> Mem {
    let imp = spec::import(TASKS, "folder", None, &[]).expect("spec");
    let mut mem = Mem::demo();
    chains::import(&mut mem, "DEMO", &imp.toml, None).expect("imports");
    mem
}

fn card(mem: &Mem, title: &str) -> Ticket {
    let t = mem.view.tickets.iter().find(|t| t.title == title);
    mem.ticket_by_id(t.expect("card").id)
}

fn items(mem: &Mem) -> Vec<(InboxKind, String)> {
    inbox::load(mem, mem.project.id)
        .expect("inbox")
        .into_iter()
        .map(|i| (i.kind, i.ticket.clone().expect("card").to_string()))
        .collect()
}

fn open_texts(mem: &Mem, title: &str) -> Vec<String> {
    let id = card(mem, title).id;
    let mut subs: Vec<_> = mem.subtasks.iter().filter(|s| s.ticket_id == id).collect();
    subs.sort_by_key(|s| s.position);
    subs.iter()
        .filter(|s| !s.done)
        .map(|s| s.text.clone())
        .collect()
}

/// What `tok chain export` would write back: the chain's cards in order, with their subtasks.
fn exported(mem: &Mem) -> String {
    let mut cards = Vec::new();
    for node in mem.view.topo_order() {
        let NodeRef::Ticket(id) = node else { continue };
        let t = mem.ticket_by_id(id);
        let mut subs: Vec<_> = mem.subtasks.iter().filter(|s| s.ticket_id == id).collect();
        subs.sort_by_key(|s| s.position);
        cards.push(ExportCard {
            title: t.title,
            prompt: t.prompt,
            subtasks: subs
                .into_iter()
                .map(|s| ExportSubtask {
                    text: s.text.clone(),
                    done: s.done,
                })
                .collect(),
        });
    }
    spec::patch(TASKS, &cards).expect("patch")
}

#[test]
fn the_import_leaves_one_item_for_the_card_with_open_subtasks() {
    let mem = imported();
    assert_eq!(items(&mem), [(InboxKind::Leftovers, "DEMO-3".to_owned())]);
    let item = &inbox::load(&mem, mem.project.id).expect("inbox")[0];
    assert_eq!(item.subtasks, ["T002 wire it", "T003 test it"]);
}

#[test]
fn moving_puts_the_open_subtasks_in_a_card_at_the_end_and_the_export_is_unchanged() {
    let mut mem = imported();
    let setup = card(&mem, "Setup");
    resolve(&mut mem, &setup, LeftoverAction::Move).expect("move");

    let leftovers = card(&mem, "Leftovers from Setup");
    assert_eq!(
        open_texts(&mem, "Leftovers from Setup"),
        ["T002 wire it", "T003 test it"]
    );
    assert_eq!(open_texts(&mem, "Setup"), Vec::<String>::new());
    // At the end of the chain: it waits for the last card, so it is the last in order.
    let order: Vec<_> = mem.view.topo_order();
    assert_eq!(order.last(), Some(&NodeRef::Ticket(leftovers.id)));
    let build = card(&mem, "Build");
    assert!(
        mem.view.edges.iter().any(|e| {
            e.from == NodeRef::Ticket(build.id) && e.to == NodeRef::Ticket(leftovers.id)
        })
    );
    assert_eq!(items(&mem), []);
    assert_eq!(
        exported(&mem),
        TASKS,
        "the spec folder round trip is byte-identical"
    );
}

#[test]
fn closing_marks_the_subtasks_done_and_leaves_a_note_in_the_journal() {
    let mut mem = imported();
    let setup = card(&mem, "Setup");
    resolve(&mut mem, &setup, LeftoverAction::Close).expect("close");

    assert_eq!(open_texts(&mem, "Setup"), Vec::<String>::new());
    assert_eq!(items(&mem), []);
    let note = mem
        .comments
        .iter()
        .find(|c| c.kind == CommentKind::Decision)
        .expect("a note");
    assert_eq!(note.body.gist, "Closed 2 leftover tasks of DEMO-3");
    assert!(
        mem.kinds().contains(&"comment.added".to_owned()),
        "{:?}",
        mem.kinds()
    );
    // Only the two checkboxes flip; every other byte stays.
    let want = TASKS
        .replace("- [ ] T002", "- [x] T002")
        .replace("- [ ] T003", "- [x] T003");
    assert_eq!(exported(&mem), want);
}

#[test]
fn leaving_dismisses_the_item_for_good_and_changes_no_subtask() {
    let mut mem = imported();
    let setup = card(&mem, "Setup");
    resolve(&mut mem, &setup, LeftoverAction::Leave).expect("leave");

    assert_eq!(items(&mem), []);
    assert_eq!(open_texts(&mem, "Setup"), ["T002 wire it", "T003 test it"]);
    let build = card(&mem, "Build");
    let mark = build.done_out_of_order.expect("mark stays");
    assert_eq!(mark.dismissed, mark.behind);
    assert_eq!(exported(&mem), TASKS);
    // It is decided once.
    let again = resolve(&mut mem, &setup, LeftoverAction::Leave).expect_err("decided");
    assert_eq!(again.kind, ErrorKind::Conflict);
}

#[test]
fn a_card_without_leftovers_and_an_agent_are_refused_and_nothing_changes() {
    let mut mem = imported();
    let build = card(&mem, "Build");
    let e = resolve(&mut mem, &build, LeftoverAction::Move).expect_err("nothing left");
    assert_eq!(
        (e.kind, e.message),
        (
            ErrorKind::Conflict,
            LeftoverError::NothingLeft(build.key.clone()).to_string()
        )
    );
    let setup = card(&mem, "Setup");
    mem.actor = toknila_core::Actor::Agent;
    let events = mem.events.len();
    let e = resolve(&mut mem, &setup, LeftoverAction::Close).expect_err("person only");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(mem.events.len(), events);
    assert_eq!(card(&mem, "Setup").status, TicketStatus::Ready);
}
