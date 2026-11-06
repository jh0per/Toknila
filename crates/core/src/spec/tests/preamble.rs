//! Tasks before the first `##` heading are one card, titled after the spec.

use super::*;

const TEXT: &str = "# Tasks: Tasks Title\n\nSome words.\n\n- [x] T001 first\n- [ ] T002 second\n\n## Phase 1: Build\n\n- [ ] T003 third\n";

#[test]
fn tasks_before_the_first_heading_are_a_card_without_a_phase_number() {
    let spec = parse(TEXT).expect("parses");
    assert_eq!(spec.title.as_deref(), Some("Tasks Title"));
    assert_eq!(spec.phases.len(), 2);
    let first = &spec.phases[0];
    assert_eq!((first.number, first.story, first.line), (None, false, 5));
    assert_eq!(first.title, "");
    let tasks: Vec<_> = first
        .tasks
        .iter()
        .map(|t| (t.text.as_str(), t.done))
        .collect();
    assert_eq!(tasks, [("T001 first", true), ("T002 second", false)]);
    assert_eq!(spec.phases[1].number, Some(1));
}

#[test]
fn the_card_is_titled_by_tasks_title_then_spec_title_then_folder() {
    let title = |text: &str, spec: Option<&str>| {
        let imp = import(text, "the-folder", spec, &[]).expect("imports");
        plan::parse(&imp.toml).expect("valid").tickets[0]
            .title
            .clone()
    };
    assert_eq!(title(TEXT, Some("Spec Title")), "Tasks Title");
    let bare = "- [ ] T001 a\n";
    assert_eq!(title(bare, Some("Spec Title")), "Spec Title");
    assert_eq!(title(bare, None), "the-folder");
    assert_eq!(title("# Tasks:\n\n- [ ] T001 a\n", None), "the-folder");
}

#[test]
fn later_phases_wait_for_the_card_and_it_has_the_ticks() {
    let imp = import(TEXT, "f", None, &[]).expect("imports");
    assert_eq!((imp.cards, imp.tasks, imp.done_cards), (2, 3, 0));
    let plan = plan::parse(&imp.toml).expect("valid");
    assert_eq!(plan.tickets[0].subtasks, ["T001 first", "T002 second"]);
    assert_eq!(plan.tickets[0].done_subtasks, [1]);
    assert_eq!(plan.tickets[1].title, "Build");
    assert_eq!(plan.edges.len(), 1);
}

#[test]
fn a_task_after_a_later_title_heading_is_still_outside_a_phase() {
    let e = err("## Phase 1: A\n- [ ] T001 ok\n\n# Notes\n\n- [ ] T002 stray\n");
    assert_eq!((e.line, e.kind), (6, SpecErrorKind::TaskOutsidePhase));
}

#[test]
fn export_right_after_import_is_byte_identical_with_a_preamble_card() {
    for text in [
        TEXT.to_owned(),
        crlf(TEXT),
        "- [ ] T001 only\n- [x] T002 tasks\n".to_owned(),
        "# Tasks: X\n\n- [ ] no id one\n- [x] no id two\n\n## Phase 2: B\n- [ ] T9 b\n".to_owned(),
    ] {
        assert_eq!(patch(&text, &cards_of(&text)).expect("patch"), text);
    }
}

#[test]
fn ticking_a_preamble_subtask_changes_exactly_one_byte() {
    let mut cards = cards_of(TEXT);
    cards[0].subtasks[1].done = true;
    let out = patch(TEXT, &cards).expect("patch");
    assert_eq!(out, TEXT.replace("- [ ] T002", "- [x] T002"));
}
