//! Instruction delivery to a live agent: typed to an idle one, held for the Stop hook of a working
//! one, delivered once.

use toknila_core::instructions::terminal::Size;
use toknila_core::{CommentBody, CommentKind};

use super::*;
use crate::instructions;

fn running(mem: &mut Mem) {
    recover(mem).expect("settle");
    settle(mem);
}

fn comment(mem: &mut Mem, kind: CommentKind, gist: &str) {
    let t = a(mem);
    let body = CommentBody {
        gist: gist.to_owned(),
        ..Default::default()
    };
    tickets::comment(mem, &t, kind, body).expect("comment");
    settle(mem);
}

fn instruct(mem: &mut Mem, gist: &str) {
    comment(mem, CommentKind::Instruction, gist);
}

fn claim(mem: &mut Mem) -> instructions::Claimed {
    let t = a(mem);
    instructions::claim(mem, &t).expect("claim")
}

fn delivered(mem: &Mem) -> usize {
    mem.kinds()
        .iter()
        .filter(|k| *k == "comment.delivered")
        .count()
}

#[test]
fn an_instruction_for_an_idle_agent_is_typed_and_marked_delivered_once() {
    let mut mem = with_agents();
    running(&mut mem);
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: true,
    });
    instruct(&mut mem, "use rustls");
    let written = mem.runner.written.borrow().clone();
    assert_eq!(written[0], b"# [instruction] use rustls");
    assert_eq!(written[1], b"\r");
    assert_eq!(delivered(&mem), 1);
    // Nothing is left for a Stop hook to hand out a second time.
    let claimed = claim(&mut mem);
    assert_eq!(claimed.count, 0);
    assert_eq!(delivered(&mem), 1);
}

#[test]
fn an_instruction_for_a_working_agent_with_hooks_waits_for_the_stop_hook() {
    let mut mem = with_agents();
    running(&mut mem);
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: false,
    });
    instruct(&mut mem, "first");
    instruct(&mut mem, "second");
    assert!(
        mem.runner.written.borrow().is_empty(),
        "nothing typed mid-turn"
    );
    assert_eq!(delivered(&mem), 0);

    let claimed = claim(&mut mem);
    assert_eq!(claimed.count, 2);
    assert!(
        claimed
            .reason
            .starts_with("2 new instructions from a person")
    );
    assert!(claimed.reason.find("first") < claimed.reason.find("second"));
    assert_eq!(delivered(&mem), 2);

    let again = claim(&mut mem);
    assert_eq!((again.count, again.reason.as_str()), (0, ""));
    assert_eq!(delivered(&mem), 2, "claimed once");
}

#[test]
fn without_hooks_a_working_agent_is_typed_to() {
    let mut mem = with_agents();
    running(&mut mem);
    mem.runner.reach.set(RunnerState {
        hooks: false,
        idle: false,
    });
    instruct(&mut mem, "now");
    assert_eq!(mem.runner.written.borrow().len(), 2);
    assert_eq!(delivered(&mem), 1);
}

#[test]
fn an_instruction_for_a_ticket_without_a_live_runner_stays_pending() {
    let mut mem = with_agents();
    running(&mut mem);
    mem.runner.live.borrow_mut().clear();
    instruct(&mut mem, "later");
    assert_eq!(delivered(&mem), 0);
    let claimed = claim(&mut mem);
    assert_eq!(claimed.count, 1);
}

#[test]
fn only_a_persons_instructions_are_delivered() {
    let mut mem = with_agents();
    running(&mut mem);
    comment(&mut mem, CommentKind::Report, "done so far");
    let claimed = claim(&mut mem);
    assert_eq!(claimed.count, 0);
    assert!(mem.runner.written.borrow().is_empty());
}

#[test]
fn a_resize_reaches_the_live_terminal_only() {
    let mut mem = with_agents();
    running(&mut mem);
    let t = a(&mem);
    let size = Size { cols: 90, rows: 30 };
    mem.runner.resize(t.id, size).expect("resize");
    assert_eq!(mem.runner.sized.borrow().as_slice(), [size]);
    mem.runner.live.borrow_mut().clear();
    assert!(mem.runner.resize(t.id, size).is_err());
}
