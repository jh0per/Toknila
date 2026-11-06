//! A card whose tasks are all checked but which sits behind a card that is not.

use super::*;

/// Phase 2 has two open tasks; phases 3 and 4 are fully checked; phase 1 is in order.
const TEXT: &str = "# Tasks: X\n\n## Phase 1: A\n- [x] T001 a\n\n## Phase 2: B\n- [x] T002 b\n- [ ] T003 c\n- [ ] T004 d\n\n## Phase 3: C\n- [x] T005 e\n\n## Phase 4: D\n- [x] T006 f\n";

#[test]
fn the_report_names_each_card_and_what_is_still_open_before_it() {
    let imp = import(TEXT, "f", None, &[]).expect("imports");
    let lines: Vec<String> = imp.out_of_order.iter().map(ToString::to_string).collect();
    assert_eq!(
        lines,
        [
            "phase-3 done out of order: phase-2 has 2 open subtasks (T003, T004)",
            "phase-4 done out of order: phase-2 has 2 open subtasks (T003, T004)",
        ]
    );
    assert_eq!(imp.done_cards, 3);
}

#[test]
fn an_in_order_import_reports_nothing() {
    let imp = import(SAMPLE, "f", None, &[]).expect("imports");
    assert!(imp.out_of_order.is_empty());
}

#[test]
fn a_card_without_a_phase_number_is_named_by_its_title_and_several_gaps_are_listed() {
    let text = "## Preconditions\n- [ ] T001 a\n\n## Phase 2: B\n- [ ] T002 b\n\n## Wrap up\n- [x] T003 c\n";
    let imp = import(text, "f", None, &[]).expect("imports");
    assert_eq!(
        imp.out_of_order[0].to_string(),
        "Wrap up done out of order: Preconditions has 1 open subtask (T001); phase-2 has 1 open subtask (T002)"
    );
}

#[test]
fn stories_run_in_parallel_so_a_done_story_is_not_behind_an_open_sibling() {
    let text = "## Phase 1: A\n- [x] T001 a\n\n## Phase 3: User Story 1 - X\n- [ ] T002 b\n\n## Phase 4: User Story 2 - Y\n- [x] T003 c\n";
    let imp = import(text, "f", None, &[]).expect("imports");
    assert!(imp.out_of_order.is_empty());
}

#[test]
fn the_plan_finds_the_same_cards_from_the_chain_file() {
    let imp = import(TEXT, "f", None, &[]).expect("imports");
    let plan = plan::parse(&imp.toml).expect("valid");
    assert_eq!(plan.out_of_order(), [vec![], vec![], vec![1], vec![1]]);
}
