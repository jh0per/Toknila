use super::*;
use crate::plan;

const SAMPLE: &str = include_str!("../../../../examples/specs/001-sample-feature/tasks.md");

fn crlf(s: &str) -> String {
    s.replace('\n', "\r\n")
}

fn docs() -> Vec<String> {
    ["spec.md", "plan.md", "contracts/", "tasks.md"]
        .map(String::from)
        .to_vec()
}

fn err(text: &str) -> SpecError {
    parse(text).expect_err("should be rejected")
}

#[test]
fn phases_tasks_and_prompts_are_read() {
    let spec = parse(SAMPLE).expect("sample parses");
    assert_eq!(spec.title.as_deref(), Some("Sample Feature"));
    // The free-text sections (Dependencies, ...) are sections without tasks, not cards.
    let spec = SpecTasks {
        phases: spec
            .phases
            .into_iter()
            .filter(|p| !p.tasks.is_empty())
            .collect(),
        ..spec
    };
    let titles: Vec<_> = spec.phases.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Setup",
            "Foundational State",
            "US1: Choose a creation path",
            "US2: Resume a draft",
            "US3: Share a result",
            "Polish, Integration, and Release Evidence",
        ]
    );
    let story: Vec<_> = spec.phases.iter().map(|p| p.story).collect();
    assert_eq!(story, [false, false, true, true, true, false]);
    let us1 = &spec.phases[2];
    assert_eq!(
        us1.prompt,
        "**Goal**: A person picks how to start.\n**Independent Test**: Open the home page and see the choices."
    );
    assert_eq!(us1.tasks.len(), 2);
    assert!(us1.tasks[0].done && !us1.tasks[1].done);
    assert_eq!(
        us1.tasks[1].text,
        "T006 [P] [US1] Implement the hero wall in `src/wall.rs`"
    );
    assert_eq!(spec.phases[0].tasks[1].text.split(' ').next(), Some("T002"));
}

#[test]
fn crlf_file_parses_the_same() {
    let lf = parse(SAMPLE).expect("lf");
    let cr = parse(&crlf(SAMPLE)).expect("crlf");
    type Shape = Vec<(String, String, Vec<(String, bool)>)>;
    let shape = |s: &SpecTasks| -> Shape {
        s.phases
            .iter()
            .map(|p| {
                let tasks = p.tasks.iter().map(|t| (t.text.clone(), t.done)).collect();
                (p.title.clone(), p.prompt.clone(), tasks)
            })
            .collect()
    };
    assert_eq!(shape(&lf), shape(&cr));
}

#[test]
fn mark_offsets_point_at_the_checkbox() {
    for text in [SAMPLE.to_owned(), crlf(SAMPLE)] {
        let spec = parse(&text).expect("parses");
        for t in spec.phases.iter().flat_map(|p| &p.tasks) {
            let c = text.as_bytes()[t.mark_offset];
            assert_eq!(c == b'x', t.done, "line {}", t.line);
            assert!(c == b'x' || c == b' ');
        }
    }
}

#[test]
fn a_malformed_task_line_names_its_line() {
    let e = err("## Phase 1: A\n\n- [ ] T001 fine\n- [y] T002 bad mark\n");
    assert_eq!((e.line, e.kind), (4, SpecErrorKind::MalformedTask));
    let e = err("## Phase 1: A\n- [ ]\n");
    assert_eq!((e.line, e.kind), (2, SpecErrorKind::MalformedTask));
    let e = err("## Phase 1: A\n- [x]T001 no space\n");
    assert_eq!((e.line, e.kind), (2, SpecErrorKind::MalformedTask));
}

#[test]
fn a_task_outside_a_phase_is_an_error() {
    let e = err("## Phase 1: A\n- [ ] T001 ok\n\n# Notes\n\n- [ ] T002 stray\n");
    assert_eq!((e.line, e.kind), (6, SpecErrorKind::TaskOutsidePhase));
}

#[test]
fn another_section_with_tasks_is_a_card_and_sub_headings_stay_in_their_phase() {
    let text = "# Tasks: X\n\n## Preconditions\n\n- [x] T001 Read the docs\n\n## Phase 1: Build\n\n### Tests first\n\n- [ ] **A1** bold id\n\n### Implementation\n\n- [ ] T002 code\n\n## Notes\n\nfree text\n";
    let spec = parse(text).expect("parses");
    let got: Vec<_> = spec
        .phases
        .iter()
        .map(|p| (p.title.as_str(), p.number, p.tasks.len()))
        .collect();
    assert_eq!(
        got,
        [
            ("Preconditions", None, 1),
            ("Build", Some(1), 2),
            ("Notes", None, 0)
        ]
    );
    assert_eq!(spec.phases[1].prompt, "### Tests first");
    let imp = import(text, "x", None, &[]).expect("imports");
    assert_eq!((imp.cards, imp.tasks), (2, 3));
}

#[test]
fn links_and_fenced_code_are_not_tasks() {
    let text =
        "## Phase 1: A\n\n- [docs](x) is a link\n```\n- [?] not a task\n```\n- [ ] T001 real\n";
    let spec = parse(text).expect("parses");
    assert_eq!(spec.phases[0].tasks.len(), 1);
    assert!(spec.phases[0].prompt.contains("- [docs](x)"));
}

#[test]
fn heading_variants() {
    let text = "\u{feff}# Tasks: Bom Title\n## phase 7 - Lower case\n- [ ] a\n## Phase 8\n- [ ] b\n## Phase 9: User Story 10 \u{2013} Dash (P1)\n- [ ] c\n## Phase 10: User Stories overview\n- [ ] d\n";
    let spec = parse(text).expect("parses");
    assert_eq!(spec.title.as_deref(), Some("Bom Title"));
    let got: Vec<_> = spec
        .phases
        .iter()
        .map(|p| (p.number, p.title.as_str(), p.story))
        .collect();
    assert_eq!(
        got,
        [
            (Some(7), "Lower case", false),
            (Some(8), "Phase 8", false),
            (Some(9), "US10: Dash (P1)", true),
            (Some(10), "User Stories overview", false),
        ]
    );
}

#[test]
fn dependency_rule() {
    // Setup, Foundational, three stories, Polish.
    let deps = phase_deps(&[false, false, true, true, true, false]);
    assert_eq!(
        deps,
        [vec![], vec![0], vec![1], vec![1], vec![1], vec![2, 3, 4]]
    );
    // Stories first; a plain phase after them waits for them.
    assert_eq!(
        phase_deps(&[true, true, false]),
        [vec![], vec![], vec![0, 1]]
    );
    // Plain phases form a line; the next group starts after the last plain one.
    assert_eq!(
        phase_deps(&[false, false, false, true, false, true]),
        [vec![], vec![0], vec![1], vec![2], vec![3], vec![4]]
    );
    assert!(phase_deps(&[]).is_empty());
}

#[test]
fn import_builds_a_chain_file_the_chain_parser_accepts() {
    let imp = import(SAMPLE, "001-sample-feature", None, &docs()).expect("imports");
    assert_eq!((imp.cards, imp.tasks, imp.done_cards), (6, 11, 2));
    let plan = plan::parse(&imp.toml).expect("a valid chain file");
    assert_eq!(plan.chain.name, "001-sample-feature");
    assert!(!plan.chain.autostart);
    assert_eq!(
        plan.chain.goal,
        "Sample Feature\n\nDocuments:\n- spec.md\n- plan.md\n- contracts/\n- tasks.md"
    );
    let done: Vec<_> = plan.tickets.iter().map(|t| t.done).collect();
    assert_eq!(done, [true, true, false, false, false, false]);
    assert_eq!(plan.tickets[2].done_subtasks, [1]);
    assert_eq!(plan.tickets[2].subtasks.len(), 2);
    let edges: Vec<_> = plan
        .edges
        .iter()
        .map(|e| format!("{:?}>{:?}", e.from, e.to))
        .collect();
    assert_eq!(edges.len(), 1 + 3 + 3);
    // The chain JSON an importer sees, pinned.
    let json = serde_json::to_string_pretty(&plan).expect("json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/spec/imported_plan.json");
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, format!("{json}\n")).expect("write snapshot");
    }
    let want = std::fs::read_to_string(&path).expect("snapshot; TOKNILA_UPDATE_SNAPSHOTS=1");
    assert_eq!(format!("{json}\n"), want.replace("\r\n", "\n"));
}

#[test]
fn import_errors_are_typed() {
    assert_eq!(
        import("# Tasks: X\n## Notes\ntext\n", "x", None, &[])
            .expect_err("no tasks")
            .kind,
        SpecErrorKind::NoTasks
    );
    assert_eq!(
        import(SAMPLE, " ", None, &[]).expect_err("name").kind,
        SpecErrorKind::EmptyName
    );
    let e = import("## Phase 1: A\n- [q] T1\n", "x", None, &[]).expect_err("bad");
    assert_eq!((e.line, e.kind), (2, SpecErrorKind::MalformedTask));
}

#[test]
fn a_phase_without_tasks_is_not_a_card() {
    let text =
        "## Phase 1: A\n- [ ] T001 a\n## Phase 2: Empty\nonly words\n## Phase 3: B\n- [ ] T002 b\n";
    let imp = import(text, "x", None, &[]).expect("imports");
    assert_eq!(imp.cards, 2);
    let plan = plan::parse(&imp.toml).expect("valid");
    assert_eq!(plan.edges.len(), 1);
}

fn card(title: &str, subs: &[(&str, bool)]) -> ExportCard {
    ExportCard {
        title: title.into(),
        prompt: String::new(),
        subtasks: subs
            .iter()
            .map(|(t, d)| ExportSubtask {
                text: (*t).into(),
                done: *d,
            })
            .collect(),
    }
}

/// The cards an import of `text` gives, as the chain holds them.
fn cards_of(text: &str) -> Vec<ExportCard> {
    parse(text)
        .expect("parses")
        .phases
        .into_iter()
        .filter(|p| !p.tasks.is_empty())
        .map(|p| ExportCard {
            title: p.title,
            prompt: p.prompt,
            subtasks: p
                .tasks
                .into_iter()
                .map(|t| ExportSubtask {
                    text: t.text,
                    done: t.done,
                })
                .collect(),
        })
        .collect()
}

#[test]
fn export_right_after_import_is_byte_identical() {
    for text in [
        SAMPLE.to_owned(),
        crlf(SAMPLE),
        SAMPLE.trim_end().to_owned(),
    ] {
        assert_eq!(patch(&text, &cards_of(&text)).expect("patch"), text);
    }
}

#[test]
fn export_twice_changes_nothing_more() {
    let mut cards = cards_of(SAMPLE);
    cards[3].subtasks[0].done = true;
    cards.push(ExportCard {
        title: "Wildcard".into(),
        prompt: "Why it exists.".into(),
        subtasks: vec![
            ExportSubtask {
                text: "first thing".into(),
                done: false,
            },
            ExportSubtask {
                text: "second thing".into(),
                done: true,
            },
        ],
    });
    let once = patch(SAMPLE, &cards).expect("first");
    let twice = patch(&once, &cards).expect("second");
    assert_eq!(once, twice);
    // The appended phase is read back as a card of its own: still no change.
    assert_eq!(patch(&once, &cards_of(&once)).expect("third"), once);
}

#[test]
fn ticking_one_subtask_changes_exactly_one_line() {
    for text in [SAMPLE.to_owned(), crlf(SAMPLE)] {
        let mut cards = cards_of(&text);
        assert!(!cards[3].subtasks[1].done);
        cards[3].subtasks[1].done = true;
        let out = patch(&text, &cards).expect("patch");
        let changed: Vec<_> = text
            .split_inclusive('\n')
            .zip(out.split_inclusive('\n'))
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(text.len(), out.len());
        assert_eq!(changed.len(), 1);
        assert!(changed[0].0.starts_with("- [ ] T008 "));
        assert!(changed[0].1.starts_with("- [x] T008 "));
    }
}

#[test]
fn unticking_flips_back() {
    let mut cards = cards_of(SAMPLE);
    cards[0].subtasks[0].done = false;
    let out = patch(SAMPLE, &cards).expect("patch");
    assert!(out.contains("- [ ] T001 Confirm"));
    assert_eq!(out.len(), SAMPLE.len());
}

#[test]
fn new_cards_are_appended_with_numbering_after_the_highest_ids() {
    let mut cards = cards_of(SAMPLE);
    cards.push(card(
        "Approved wildcard",
        &[("investigate flake", true), ("T500 own id", false)],
    ));
    let out = patch(SAMPLE, &cards).expect("patch");
    assert!(
        out.starts_with(SAMPLE),
        "the original bytes stay as they were"
    );
    assert_eq!(
        &out[SAMPLE.len()..],
        "\n## Phase 7: Approved wildcard\n\n- [x] T012 investigate flake\n- [ ] T500 own id\n"
    );
}

#[test]
fn appending_follows_the_line_ending_and_adds_a_missing_final_newline() {
    let text = crlf(SAMPLE.trim_end());
    let cards = vec![card("Extra", &[("one", false)])];
    let out = patch(&text, &cards).expect("patch");
    assert!(out.starts_with(&text));
    assert_eq!(
        &out[text.len()..],
        "\r\n\r\n## Phase 7: Extra\r\n\r\n- [ ] T012 one\r\n"
    );
    assert!(!out[text.len()..].replace("\r\n", "").contains('\n'));
}

#[test]
fn nothing_is_deleted_when_the_chain_lacks_cards_or_subtasks() {
    let cards = vec![card("Setup", &[("T001 Confirm the accepted scope", true)])];
    let out = patch(SAMPLE, &cards).expect("patch");
    assert_eq!(out, SAMPLE);
    assert_eq!(patch(SAMPLE, &[]).expect("empty"), SAMPLE);
}

#[test]
fn a_file_with_a_malformed_line_is_not_patched() {
    let e = patch("## Phase 1: A\n- [z] T001\n", &[]).expect_err("bad file");
    assert_eq!(e.kind, SpecErrorKind::MalformedTask);
}

#[test]
fn tasks_without_ids_match_by_text() {
    let text = "## Phase 1: A\n- [ ] write docs\n- [ ] write docs\n";
    let cards = vec![card("A", &[("write docs", true), ("write docs", false)])];
    assert_eq!(
        patch(text, &cards).expect("patch"),
        "## Phase 1: A\n- [x] write docs\n- [ ] write docs\n"
    );
}

mod out_of_order;
mod preamble;
