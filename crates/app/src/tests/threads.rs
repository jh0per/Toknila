//! Threads through the use cases: agent and person open and post, only a person decides, and the
//! agent's hand-in waits for every thread.

use toknila_core::agent::AgentEvent;
use toknila_core::{
    Actor, ArtifactFormat, MessageKind, Phase, Thread, ThreadKind, ThreadState, TicketStatus,
    thread,
};

use super::Mem;
use crate::recovery::recover;
use crate::threads::{self, NewVersion};
use crate::{ErrorKind, agent};

/// DEMO-1 (A) running, DEMO-2 (B) waiting for it; commands run as the agent.
fn running_demo() -> Mem {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    mem.events.clear();
    mem.actor = Actor::Agent;
    mem
}

fn open_design(mem: &mut Mem) -> Thread {
    let a = mem.ticket_by_title("A");
    let event = AgentEvent::OpenThread {
        kind: ThreadKind::Design,
        title: "List screen".to_owned(),
    };
    agent::ticket_event(mem, &a, event).expect("open");
    mem.threads.last().expect("thread").clone()
}

fn version(body: &str) -> NewVersion {
    NewVersion {
        format: ArtifactFormat::Mermaid,
        body: body.to_owned(),
        message: None,
    }
}

fn finish(mem: &mut Mem) -> crate::AppResult<toknila_core::TicketId> {
    let a = mem.ticket_by_title("A");
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(mem, &a, AgentEvent::Finished { summary })
}

#[test]
fn an_agent_opens_a_thread_and_numbers_come_per_ticket() {
    let mut mem = running_demo();
    let first = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let second = threads::open(&mut mem, &a, ThreadKind::Flow, "Login flow").expect("second");
    assert_eq!(
        (first.number, first.state, first.opened_by, second.number),
        (1, ThreadState::Open, Actor::Agent, 2)
    );
    assert_eq!(mem.kinds(), ["thread.opened", "thread.opened"]);
    assert_eq!(
        threads::find(&mem, &a, "#2").expect("by number").id,
        second.id
    );
    let by_id = threads::find(&mem, &a, &first.id.to_string()).expect("by id");
    assert_eq!(by_id.number, 1);
    let missing = threads::find(&mem, &a, "9").expect_err("none");
    assert_eq!(
        (missing.kind, missing.message.as_str()),
        (ErrorKind::NotFound, "thread 9 of DEMO-1 not found")
    );
    let b = mem.ticket_by_title("B");
    assert_eq!(
        threads::find(&mem, &b, "1").expect_err("other ticket").kind,
        ErrorKind::NotFound
    );
}

#[test]
fn versions_are_numbered_and_kept_while_the_agent_cannot_approve() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let v1 = threads::post_version(&mut mem, &a, &th, version("graph TD; A-->B")).expect("v1");
    let v2 = threads::post_version(&mut mem, &a, &th, version("graph TD; A-->C")).expect("v2");
    assert_eq!((v1.number, v2.number, v2.author), (1, 2, Actor::Agent));
    assert_eq!(
        mem.versions[0].body, "graph TD; A-->B",
        "version 1 is untouched"
    );
    let e = threads::approve(&mut mem, &a, &th).expect_err("agent");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(
        e.message,
        "only a person can approve a thread; this came from Agent"
    );
    let e = threads::request_changes(&mut mem, &a, &th, "more").expect_err("agent");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(mem.messages.len(), 0, "a refused request stores nothing");
    assert_eq!(
        mem.kinds(),
        [
            "thread.opened",
            "thread.version_posted",
            "thread.version_posted"
        ]
    );
}

#[test]
fn the_hand_in_is_refused_until_every_thread_is_approved() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let before = mem.events.len();
    let e = finish(&mut mem).expect_err("open thread");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(
        e.message,
        "cannot hand in DEMO-1: 1 open: #1 \"List screen\" (Design)"
    );
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert_eq!((mem.events.len(), mem.comments.len()), (before, 0));
    assert_eq!(thread::phase(&mem.threads), toknila_core::Phase::Design);

    mem.actor = Actor::Human;
    threads::approve(&mut mem, &a, &th).expect("approve");
    mem.actor = Actor::Agent;
    assert_eq!(thread::phase(&mem.threads), Phase::Implement);
    finish(&mut mem).expect("allowed now");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Review);
}

#[test]
fn approval_pins_the_latest_version_and_a_new_version_reopens() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    threads::post_version(&mut mem, &a, &th, version("v1")).expect("v1");
    mem.actor = Actor::Human;
    threads::request_changes(&mut mem, &a, &th, "add an error state").expect("changes");
    assert_eq!(mem.messages[0].kind, MessageKind::ChangesRequested);
    assert_eq!(
        mem.threads[0].state,
        ThreadState::Open,
        "changes keep it open"
    );
    mem.actor = Actor::Agent;
    threads::post_version(&mut mem, &a, &th, version("v2")).expect("v2");
    mem.actor = Actor::Human;
    let approved = threads::approve(&mut mem, &a, &th).expect("approve");
    assert_eq!(
        (approved.state, approved.approved_version),
        (ThreadState::Approved, Some(2))
    );
    let e = threads::approve(&mut mem, &a, &approved).expect_err("twice");
    assert_eq!(e.message, "thread is already approved");

    mem.events.clear();
    mem.actor = Actor::Agent;
    let th = threads::find(&mem, &a, "1").expect("fresh");
    threads::post_version(&mut mem, &a, &th, version("v3")).expect("v3");
    let reopened = threads::find(&mem, &a, "1").expect("fresh");
    assert_eq!(
        (reopened.state, reopened.approved_version),
        (ThreadState::Open, None)
    );
    assert_eq!(mem.kinds(), ["thread.version_posted", "thread.reopened"]);

    // A person's request for changes reopens an approved thread too.
    mem.actor = Actor::Human;
    threads::approve(&mut mem, &a, &reopened).expect("again");
    let th = threads::find(&mem, &a, "1").expect("fresh");
    mem.events.clear();
    threads::request_changes(&mut mem, &a, &th, "one more thing").expect("changes");
    assert_eq!(mem.threads[0].state, ThreadState::Open);
    assert_eq!(mem.kinds(), ["thread.message_posted", "thread.reopened"]);
}

#[test]
fn a_thread_without_an_artifact_can_be_approved_in_words() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    threads::post_message(&mut mem, &a, &th, "no screen needed, it is a CLI flag").expect("msg");
    mem.actor = Actor::Human;
    let approved = threads::approve(&mut mem, &a, &th).expect("approve");
    assert_eq!(
        (approved.state, approved.approved_version),
        (ThreadState::Approved, None)
    );
    assert_eq!(
        mem.kinds(),
        ["thread.opened", "thread.message_posted", "thread.approved"]
    );
}

#[test]
fn bad_input_is_a_typed_error_and_stores_nothing() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let big = "x".repeat(thread::MAX_ARTIFACT_BYTES + 1);
    let e = threads::post_version(&mut mem, &a, &th, version(&big)).expect_err("too big");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    assert_eq!(e.message, "artifact is 524289 bytes, the limit is 524288");
    let e = threads::post_message(&mut mem, &a, &th, " ").expect_err("empty");
    assert_eq!(e.message, "message needs a text");
    let e = threads::open(&mut mem, &a, ThreadKind::Other, " ").expect_err("title");
    assert_eq!(e.message, "thread title is empty");
    assert_eq!(
        (mem.versions.len(), mem.messages.len(), mem.threads.len()),
        (0, 0, 1)
    );
}

#[test]
fn the_agent_posts_through_its_events_by_number() {
    let mut mem = running_demo();
    open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let msg = AgentEvent::ThreadMessage {
        thread: "1".to_owned(),
        text: "empty, loading, error, content".to_owned(),
    };
    agent::ticket_event(&mut mem, &a, msg).expect("message");
    let ver = AgentEvent::ThreadVersion {
        thread: "1".to_owned(),
        format: ArtifactFormat::Markdown,
        body: "# List".to_owned(),
    };
    agent::ticket_event(&mut mem, &a, ver).expect("version");
    assert_eq!((mem.messages.len(), mem.versions[0].number), (1, 1));
    let unknown = AgentEvent::ThreadMessage {
        thread: "7".to_owned(),
        text: "hi".to_owned(),
    };
    let e = agent::ticket_event(&mut mem, &a, unknown).expect_err("unknown");
    assert_eq!(e.kind, ErrorKind::NotFound);
}

#[test]
fn a_version_can_carry_a_message_and_a_blank_one_refuses_both() {
    let mut mem = running_demo();
    let th = open_design(&mut mem);
    let a = mem.ticket_by_title("A");
    let mut input = version("v1");
    input.message = Some("first sketch".to_owned());
    threads::post_version(&mut mem, &a, &th, input).expect("both");
    assert_eq!((mem.versions.len(), mem.messages.len()), (1, 1));
    assert_eq!(
        mem.kinds(),
        [
            "thread.opened",
            "thread.version_posted",
            "thread.message_posted"
        ]
    );
    let mut blank = version("v2");
    blank.message = Some("  ".to_owned());
    let e = threads::post_version(&mut mem, &a, &th, blank).expect_err("blank");
    assert_eq!(e.message, "message needs a text");
    assert_eq!(
        (mem.versions.len(), mem.messages.len()),
        (1, 1),
        "nothing stored"
    );
}
