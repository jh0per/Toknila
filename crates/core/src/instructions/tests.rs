use super::terminal::{self, Size};
use super::*;
use crate::{Actor, CommentBody, CommentId, TicketId, Timestamp};

fn instruction(gist: &str, what: Option<&str>) -> Comment {
    Comment {
        id: CommentId::new(),
        ticket_id: TicketId::new(),
        author: Actor::Human,
        kind: CommentKind::Instruction,
        body: CommentBody {
            gist: gist.to_owned(),
            what: what.map(str::to_owned),
            ..CommentBody::default()
        },
        created_at: Timestamp(1),
        delivered_at: None,
    }
}

fn pinned_brief(acceptance: &[String]) -> RunBrief<'_> {
    RunBrief {
        key: "DEMO-3",
        title: "Generated instructions",
        prompt: "Generate the instructions for the run.\n",
        acceptance,
        check_command: None,
        setup_command: None,
        earlier: &[],
        waiting: &[],
        returned: None,
        design_first: false,
        designs: &[],
        has_designs: false,
        discussion: &[],
        epic: None,
        check: None,
        threads: &[],
        hand_in: None,
        checklist: &[],
    }
}

fn pinned_tasks() -> Vec<BriefTask> {
    let task = |position, text: &str, done, done_by| BriefTask {
        position,
        text: text.to_owned(),
        done,
        done_by,
    };
    vec![
        task(1, "Write the failing test", true, Some(Actor::Agent)),
        task(2, "Check on an iPhone", true, Some(Actor::Human)),
        task(3, "Run the gate", false, None),
    ]
}

fn assert_pinned(text: &str, name: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/instructions/snapshots")
        .join(name);
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, text).expect("write snapshot");
        return;
    }
    let want =
        std::fs::read_to_string(&path).expect("missing snapshot; TOKNILA_UPDATE_SNAPSHOTS=1");
    assert_eq!(text, want.replace("\r\n", "\n"));
}

#[test]
fn the_generated_file_is_pinned() {
    let acceptance = vec![
        "The generated file is pinned by a snapshot test".to_owned(),
        "An instruction comment is delivered and marked delivered".to_owned(),
    ];
    let tasks = pinned_tasks();
    let brief = RunBrief {
        checklist: &tasks,
        ..pinned_brief(&acceptance)
    };
    let text = generate(&brief);
    assert_pinned(&text, "run.md");
    let (acceptance_at, checklist_at) = (text.find("## Acceptance"), text.find("## Checklist"));
    assert!(
        acceptance_at < checklist_at && checklist_at.is_some(),
        "{text}"
    );
    assert!(text.contains("2. [x] Check on an iPhone (done by a person)"));
    assert!(text.contains("3. [ ] Run the gate\n"));
    assert!(!generate(&pinned_brief(&acceptance)).contains("## Checklist"));
}

#[test]
fn the_protocol_tells_the_agent_to_ask_in_the_persons_words_with_choices_as_actions() {
    let p = protocol();
    for rule in [
        "Ask one question at a time",
        "Say in one sentence what you cannot do",
        "Give `choices`: two to four short sentences the person can act on, the recommended one first",
        "Never write the options as (a)/(b) prose",
        "No task ids, no checklist positions",
        "Never ask whether to tick a task",
        "names the screen and the control as the window shows them (read the views, not only the engine), never a gesture the layout does not allow; when unsure, name the screen and let the person find the control",
    ] {
        assert!(p.contains(rule), "the protocol lacks: {rule}");
    }
}

#[test]
fn the_review_file_is_pinned() {
    let acceptance = vec!["The review file is pinned by a snapshot test".to_owned()];
    assert_pinned(&generate_review(&pinned_brief(&acceptance)), "review.md");
}

#[test]
fn the_check_command_is_to_be_run_as_given_and_files_changed_with_the_edit_tools() {
    let brief = RunBrief {
        check_command: Some("cargo xtask verify"),
        ..pinned_brief(&[])
    };
    let text = generate(&brief);
    assert!(text.contains("with no `cd` or `export` in front"), "{text}");
    assert!(text.contains("the ship is already your working directory"));
    assert!(generate(&brief).contains("Change files with the Edit and Write tools"));
    assert!(!generate_review(&brief).contains("Edit and Write tools"));
}

#[test]
fn both_roles_are_told_to_run_the_check_once_not_in_a_loop() {
    let brief = RunBrief {
        check_command: Some("cargo xtask verify"),
        ..pinned_brief(&[])
    };
    for text in [protocol().to_owned(), generate(&brief)] {
        assert!(
            text.contains("Run it once, not in a loop"),
            "the check is run once: {text}"
        );
        assert!(
            text.contains("only one check runs on this machine at a time"),
            "{text}"
        );
    }
}

#[test]
fn the_checks_section_points_at_the_migration_bullet_and_the_npm_ci_line() {
    let brief = RunBrief {
        check_command: Some("cargo xtask verify"),
        ..pinned_brief(&[])
    };
    let text = generate(&brief);
    assert!(
        text.contains("AGENTS.md's Adding a migration bullet"),
        "{text}"
    );
    assert!(text.contains("npm ci --no-audit --no-fund"), "{text}");
}

#[test]
fn both_roles_are_told_to_work_alone_and_to_quote_paths() {
    for text in [protocol(), review_protocol()] {
        assert!(text.contains("Do not start subagents or background agents"));
        assert!(text.contains("Quote every path in a shell command with double quotes"));
        assert!(text.contains("never backslash-escape a space"));
    }
}

#[test]
fn a_brief_without_acceptance_has_no_acceptance_section() {
    let text = generate(&RunBrief {
        key: "K-1",
        title: "t",
        prompt: "p",
        acceptance: &[],
        check_command: None,
        setup_command: None,
        earlier: &[],
        waiting: &[],
        returned: None,
        design_first: false,
        designs: &[],
        has_designs: false,
        discussion: &[],
        epic: None,
        check: None,
        threads: &[],
        hand_in: None,
        checklist: &[],
    });
    assert!(!text.contains("## Acceptance"));
    assert!(text.contains("## Task\n\np\n"));
}

#[test]
fn the_protocol_names_every_tool_and_the_template() {
    for tool in [
        "started",
        "subtask_done",
        "note",
        "ask_human",
        "finished",
        "crashed",
        "propose",
        "open_thread",
        "thread_message",
        "thread_version",
    ] {
        assert!(protocol().contains(&format!("`{tool}`")), "{tool}");
    }
    assert!(protocol().contains("# [instruction | report | decision]"));
}

#[test]
fn hooks_take_instructions_while_working_and_typing_otherwise() {
    assert_eq!(route(true, false), Route::WaitForHook);
    assert_eq!(route(true, true), Route::Type);
    assert_eq!(route(false, false), Route::Type);
    assert_eq!(route(false, true), Route::Type);
}

#[test]
fn typed_input_is_one_line_in_the_template() {
    let c = instruction("use rustls", Some("switch the client\nto rustls"));
    let line = typed(&c);
    assert_eq!(
        line,
        "# [instruction] use rustls **What:** switch the client to rustls"
    );
    assert!(!line.contains('\n'));
}

#[test]
fn the_hook_reason_lists_instructions_in_order() {
    let one = instruction("first", None);
    assert_eq!(
        for_hook(std::slice::from_ref(&one)),
        "# [instruction] first"
    );
    let two = instruction("second", Some("more"));
    let text = for_hook(&[one, two]);
    assert!(text.starts_with("2 new instructions from a person, in order:\n\n"));
    assert!(text.find("first").expect("first") < text.find("second").expect("second"));
    assert_eq!(for_hook(&[]), "");
}

#[test]
fn sizes_are_clamped_to_sane_bounds() {
    assert_eq!(
        terminal::clamp(100, 30),
        Size {
            cols: 100,
            rows: 30
        }
    );
    assert_eq!(terminal::clamp(0, 0), terminal::MIN);
    assert_eq!(terminal::clamp(1, 2), terminal::MIN);
    assert_eq!(terminal::clamp(100_000, 100_000), terminal::MAX);
    assert_eq!(terminal::clamp(u32::MAX, u32::MAX), terminal::MAX);
}

fn brief<'a>(check: Option<&'a str>) -> RunBrief<'a> {
    RunBrief {
        key: "DEMO-3",
        title: "t",
        prompt: "p",
        acceptance: &[],
        check_command: check,
        setup_command: None,
        earlier: &[],
        waiting: &[],
        returned: None,
        design_first: false,
        designs: &[],
        has_designs: false,
        discussion: &[],
        epic: None,
        check: None,
        threads: &[],
        hand_in: None,
        checklist: &[],
    }
}

#[test]
fn the_worker_kickoff_says_to_begin_commit_and_call_finished() {
    let text = kickoff(crate::AgentRole::Worker, &brief(None));
    assert!(text.contains("DEMO-3") && text.contains("commit") && text.contains("`finished`"));
    assert!(
        !text.contains('\n') && !text.contains("review_verdict"),
        "{text}"
    );
    assert!(!text.contains("Run `"), "no check command named: {text}");
    let text = kickoff(crate::AgentRole::Worker, &brief(Some("cargo xtask verify")));
    assert!(
        text.contains("Run `cargo xtask verify` before you hand in"),
        "{text}"
    );
}

#[test]
fn the_worker_is_told_the_setup_command_ran_and_not_to_run_it_again() {
    let with = RunBrief {
        setup_command: Some("npm ci --prefix apps/web"),
        ..brief(None)
    };
    let kick = kickoff(crate::AgentRole::Worker, &with);
    assert!(
        kick.contains("`npm ci --prefix apps/web` already ran in your workspace"),
        "{kick}"
    );
    let file = generate(&with);
    assert!(
        file.contains("## Workspace setup") && file.contains("`npm ci --prefix apps/web`"),
        "{file}"
    );
    let without = generate(&brief(None));
    assert!(!without.contains("Workspace setup"), "{without}");
    assert!(!kickoff(crate::AgentRole::Reviewer, &with).contains("already ran"));
}

#[test]
fn the_reviewer_kickoff_says_to_review_and_call_the_verdict_tool() {
    let text = kickoff(
        crate::AgentRole::Reviewer,
        &brief(Some("cargo xtask verify")),
    );
    assert!(
        text.contains("DEMO-3") && text.contains("`review_verdict`"),
        "{text}"
    );
    assert!(!text.contains("`finished`"), "{text}");
}

#[test]
fn the_worker_instructions_name_the_check_command_only_when_there_is_one() {
    let text = generate(&brief(Some("cargo xtask verify")));
    assert!(text.contains("## Checks"), "{text}");
    assert!(text.contains("`cargo xtask verify`") && text.contains("without asking"));
    assert!(!generate(&brief(None)).contains("## Checks"));
}

fn sent(gist: &str, at: i64, delivered: bool) -> Comment {
    Comment {
        created_at: Timestamp(at),
        delivered_at: delivered.then_some(Timestamp(at + 1)),
        ..instruction(gist, Some("do it"))
    }
}

#[test]
fn only_delivered_instructions_count_as_earlier_oldest_first() {
    let mut report = sent("a report", 1, true);
    report.kind = CommentKind::Report;
    let comments = [
        sent("second", 3, true),
        sent("pending", 4, false),
        report,
        sent("first", 2, true),
    ];
    let texts = earlier(&comments);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].contains("first") && texts[1].contains("second"));
}

#[test]
fn only_the_newest_earlier_instructions_are_kept() {
    let comments: Vec<Comment> = (0..15)
        .map(|i| sent(&format!("n{i:02}"), i, true))
        .collect();
    let texts = earlier(&comments);
    assert_eq!(texts.len(), 10);
    assert!(texts[0].contains("n05") && texts[9].contains("n14"));
}

#[test]
fn the_instructions_of_a_replacement_run_carry_the_earlier_ones_and_the_kickoff_says_so() {
    let acceptance = vec!["The earlier instruction is in the file".to_owned()];
    let texts = earlier(&[sent("use the Edit tool", 2, true)]);
    let tasks = pinned_tasks();
    let brief = RunBrief {
        earlier: &texts,
        waiting: &[],
        returned: None,
        checklist: &tasks,
        ..pinned_brief(&acceptance)
    };
    assert_pinned(&generate(&brief), "run_replacement.md");
    assert!(kickoff(crate::AgentRole::Worker, &brief).contains("given earlier"));
    assert!(!kickoff(crate::AgentRole::Worker, &pinned_brief(&acceptance)).contains("earlier"));
    assert!(!generate(&pinned_brief(&acceptance)).contains("given earlier"));
    assert!(!generate_review(&brief).contains("given earlier"));
}

#[test]
fn a_design_first_card_is_told_to_open_a_design_thread_before_any_code() {
    let acceptance = vec!["A design is approved before the code".to_owned()];
    let brief = RunBrief {
        design_first: true,
        ..pinned_brief(&acceptance)
    };
    assert_pinned(&generate(&brief), "run_design_first.md");
    let text = generate(&brief);
    assert!(text.contains("## Design first"));
    assert!(text.contains("`open_thread` with kind `design`"));
    assert!(text.contains("Write code only after the person approved a version"));
    let kick = kickoff(crate::AgentRole::Worker, &brief);
    assert!(kick.contains("open a design thread"), "{kick}");
    // A card that did not ask for it is told nothing about it.
    let plain = pinned_brief(&acceptance);
    assert!(!generate(&plain).contains("Design first"));
    assert!(!kickoff(crate::AgentRole::Worker, &plain).contains("design thread"));
    assert!(!generate_review(&brief).contains("Design first"));
}

#[test]
fn a_card_reads_the_goal_of_its_chain_as_the_epic() {
    let brief = RunBrief {
        epic: Some(("checkout", "Pay with saved cards.\n")),
        ..pinned_brief(&[])
    };
    let text = generate(&brief);
    assert!(
        text.contains("## Epic: checkout\n\nThis story belongs to the epic checkout."),
        "{text}"
    );
    assert!(text.contains("\n\nPay with saved cards.\n"), "{text}");
    // No goal, no section.
    let empty = RunBrief {
        epic: Some(("checkout", " ")),
        ..pinned_brief(&[])
    };
    assert!(!generate(&empty).contains("## Epic"));
    assert!(!generate(&pinned_brief(&[])).contains("## Epic"));
}

#[test]
fn a_card_with_designs_is_told_the_files_and_the_regions() {
    use super::BriefDesign;
    use crate::design::Region;
    let acceptance = vec!["x".to_owned()];
    let designs = [
        BriefDesign {
            name: "login.png".to_owned(),
            path: Some("/home/designs/p/d1.png".to_owned()),
            region: Some(Region {
                x: 20,
                y: 10,
                w: 40,
                h: 30,
            }),
        },
        BriefDesign {
            name: "frame".to_owned(),
            path: None,
            region: None,
        },
    ];
    let text = generate(&RunBrief {
        designs: &designs,
        ..pinned_brief(&acceptance)
    });
    assert!(text.contains("## Designs"), "{text}");
    assert!(
        text.contains(
            "- login.png: `/home/designs/p/d1.png`; region: x 20%, y 10%, width 40%, height 30%"
        ),
        "{text}"
    );
    assert!(text.contains("- frame: no image is kept"), "{text}");
    // Control: a card without designs has no such section.
    assert!(!generate(&pinned_brief(&acceptance)).contains("## Designs"));
}

#[test]
fn a_card_with_a_discussion_is_told_what_was_said_and_one_without_is_told_nothing() {
    let acceptance = vec!["The discussion is in the file".to_owned()];
    let said = vec![
        "person: make it stricter".to_owned(),
        "assistant proposed an edit (rejected: the card is unchanged); prompt to: p2".to_owned(),
    ];
    let with = RunBrief {
        discussion: &said,
        ..pinned_brief(&acceptance)
    };
    let text = generate(&with);
    assert!(text.contains("## Discussion with the assistant"), "{text}");
    assert!(text.contains("- person: make it stricter\n"));
    assert!(text.contains(
        "- assistant proposed an edit (rejected: the card is unchanged); prompt to: p2\n"
    ));
    assert!(!generate(&pinned_brief(&acceptance)).contains("Discussion"));
    assert!(!generate_review(&with).contains("Discussion"));
}
