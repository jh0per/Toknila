//! A worker the host stopped while its card is still running: the card still needs an agent.

use toknila_core::{BlockedReason, StatusReason, TicketStatus, Timestamp};

use super::{a, ended, finish, settle, with_agents};
use crate::external::ExternalEffect;
use crate::recovery::recover;
use crate::runs;

fn starts(mem: &super::Mem) -> usize {
    let starts = mem.deferred.iter();
    starts
        .filter(|e| matches!(e, ExternalEffect::StartRunner { .. }))
        .count()
}

#[test]
fn a_stopped_worker_on_a_running_card_without_another_run_gets_a_replacement() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    assert_eq!(mem.runner.started.borrow().len(), 1);
    // The replacement the machine asked for was lost to the race: no live worker remains.
    mem.runner.live.borrow_mut().clear();
    let e = ended(&mem, None, true);
    assert!(runs::ended(&mut mem, e).expect("ended"));
    assert_eq!(starts(&mem), 1);
    settle(&mut mem);
    assert_eq!(mem.runner.started.borrow().len(), 2);
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_stopped_worker_whose_card_has_another_live_run_is_left_alone() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let e = ended(&mem, None, true);
    assert!(!runs::ended(&mut mem, e).expect("ended"));
    assert_eq!(starts(&mem), 0);
}

#[test]
fn a_stopped_worker_on_a_card_in_review_defers_nothing() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    mem.runner.live.borrow_mut().clear();
    mem.deferred.clear();
    let before = a(&mem).status;
    assert_ne!(before, TicketStatus::Running);
    let e = ended(&mem, None, true);
    assert!(!runs::ended(&mut mem, e).expect("ended"));
    assert_eq!(starts(&mem), 0);
    assert_eq!(a(&mem).status, before);
}

#[test]
fn a_worker_stopped_right_after_it_started_blocks_the_card_instead_of_looping() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    mem.runner.live.borrow_mut().clear();
    let e = ended(&mem, None, true);
    let started = i64::try_from(e.run.id.0.timestamp_ms()).expect("ms");
    mem.now = Timestamp(started + 1_000);
    mem.deferred.clear();
    assert!(!runs::ended(&mut mem, e).expect("ended"));
    assert_eq!(starts(&mem), 0);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Blocked);
    let Some(StatusReason::Blocked { reason, .. }) = t.status_reason else {
        panic!("not blocked: {:?}", t.status_reason);
    };
    assert_eq!(
        reason,
        BlockedReason::Crashed {
            detail: "the agent ends right after it starts".to_owned()
        }
    );
}
