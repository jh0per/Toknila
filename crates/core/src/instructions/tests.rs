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

#[test]
fn the_generated_file_is_pinned() {
    let acceptance = vec![
        "The generated file is pinned by a snapshot test".to_owned(),
        "An instruction comment is delivered and marked delivered".to_owned(),
    ];
    let text = generate(&RunBrief {
        key: "DEMO-3",
        title: "Generated instructions",
        prompt: "Generate the instructions for the run.\n",
        acceptance: &acceptance,
    });
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/instructions/snapshots/run.md");
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &text).expect("write snapshot");
        return;
    }
    let want =
        std::fs::read_to_string(&path).expect("missing snapshot; TOKNILA_UPDATE_SNAPSHOTS=1");
    assert_eq!(text, want.replace("\r\n", "\n"));
}

#[test]
fn a_brief_without_acceptance_has_no_acceptance_section() {
    let text = generate(&RunBrief {
        key: "K-1",
        title: "t",
        prompt: "p",
        acceptance: &[],
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
