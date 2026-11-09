//! Subtasks added in Toknila to a card that is in the file are written under that card.

use super::*;

const TEXT: &str = "# Tasks: X\n\n## Phase 1: A\n\n- [ ] T001 a\n- [x] T002 b\n\nNotes after.\n\n## Phase 2: B\n- [ ] T003 c\n";

#[test]
fn a_new_subtask_lands_after_the_last_task_of_its_card() {
    let mut cards = cards_of(TEXT);
    cards[0].subtasks.push(ExportSubtask {
        text: "added in toknila".into(),
        done: true,
    });
    cards[1].subtasks.push(ExportSubtask {
        text: "T900 own id".into(),
        done: false,
    });
    let out = patch(TEXT, &cards).expect("patch");
    assert_eq!(
        out,
        "# Tasks: X\n\n## Phase 1: A\n\n- [ ] T001 a\n- [x] T002 b\n- [x] T004 added in toknila\n\nNotes after.\n\n## Phase 2: B\n- [ ] T003 c\n- [ ] T900 own id\n"
    );
}

#[test]
fn the_added_subtask_is_written_once_and_keeps_crlf() {
    let text = crlf(TEXT.trim_end());
    let mut cards = cards_of(&text);
    cards[1].subtasks.push(ExportSubtask {
        text: "extra".into(),
        done: false,
    });
    let once = patch(&text, &cards).expect("patch");
    assert_eq!(once, format!("{text}\r\n- [ ] T004 extra\r\n"));
    // Re-reading the patched file and exporting the same card again changes nothing.
    let mut again = cards_of(&once);
    again[1].subtasks[1].text = "extra".into();
    assert_eq!(patch(&once, &again).expect("patch"), once);
}
