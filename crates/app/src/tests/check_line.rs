//! The line the checks of a machine stand in: how many start together (`max_checks`), the load
//! gate through an injected reader, and an Accept's re-check going ahead of hand-ins. Every wait
//! has a deadline; "does not start" is a short look, as short as the poll it waits on.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use toknila_core::{LoadHold, TicketId};

use crate::checks::{CheckGate, CheckKind, CheckLine, Load, LoadReader};

const DEADLINE: Duration = Duration::from_secs(10);
const LOOK: Duration = Duration::from_millis(150);
const POLL: Duration = Duration::from_millis(10);

/// A load the test sets.
#[derive(Debug, Default)]
struct FakeLoad(Mutex<Option<Load>>);

impl FakeLoad {
    fn set(&self, one_minute: f64, cores: u32) {
        *self.0.lock().expect("load") = Some(Load { one_minute, cores });
    }
}

impl LoadReader for FakeLoad {
    fn read(&self) -> Option<Load> {
        *self.0.lock().expect("load")
    }
}

fn line(load: &Arc<FakeLoad>, max: u32) -> Arc<CheckLine> {
    let reader: Arc<dyn LoadReader> = load.clone();
    let line = Arc::new(CheckLine::new(reader).with_poll(POLL));
    line.set_max_checks(max);
    line
}

/// A check that enters the line on its own thread and keeps its turn until released.
struct Check {
    started: Receiver<()>,
    release: Sender<()>,
    handle: JoinHandle<()>,
}

impl Check {
    fn enter(line: &Arc<CheckLine>, ticket: TicketId, kind: CheckKind) -> Check {
        let (started_tx, started) = channel();
        let (release, release_rx) = channel::<()>();
        let line = line.clone();
        let handle = std::thread::spawn(move || {
            let turn = line.enter(ticket, kind);
            started_tx.send(()).ok();
            release_rx.recv_timeout(DEADLINE).ok();
            drop(turn);
        });
        Check {
            started,
            release,
            handle,
        }
    }

    fn has_started(&self) -> bool {
        self.started.recv_timeout(DEADLINE).is_ok()
    }

    fn is_waiting(&self) -> bool {
        self.started.recv_timeout(LOOK).is_err()
    }

    fn finish(self) {
        self.release.send(()).ok();
        self.handle.join().expect("the check thread ended");
    }
}

fn id() -> TicketId {
    TicketId::new()
}

#[test]
fn max_two_starts_two_checks_and_holds_the_third() {
    let load = Arc::new(FakeLoad::default());
    load.set(0.5, 8);
    let line = line(&load, 2);
    let (a, b, c) = (id(), id(), id());
    let first = Check::enter(&line, a, CheckKind::HandIn);
    assert!(first.has_started(), "the first check starts");
    let second = Check::enter(&line, b, CheckKind::HandIn);
    assert!(second.has_started(), "max 2: the second starts beside it");
    let third = Check::enter(&line, c, CheckKind::HandIn);
    assert!(third.is_waiting(), "a third waits for a place");
    let stand = line.standing();
    assert_eq!(
        (stand.place(a), stand.place(b), stand.place(c)),
        (Some((0, None)), Some((0, None)), Some((1, None))),
        "two run, the third is next in line and not held by load"
    );
    first.finish();
    assert!(third.has_started(), "a place freed: the third starts");
    second.finish();
    third.finish();
}

#[test]
fn max_one_starts_one_check_even_on_an_idle_machine() {
    let load = Arc::new(FakeLoad::default());
    load.set(0.1, 16);
    let line = line(&load, 1);
    let first = Check::enter(&line, id(), CheckKind::HandIn);
    assert!(first.has_started());
    let second = Check::enter(&line, id(), CheckKind::HandIn);
    assert!(second.is_waiting(), "max 1: the second waits");
    first.finish();
    assert!(second.has_started(), "it starts when the first is done");
    second.finish();
}

#[test]
fn the_default_is_one_check_at_a_time() {
    assert_eq!(CheckLine::default().max_checks(), 1);
    let line = CheckLine::default();
    line.set_max_checks(0);
    assert_eq!(line.max_checks(), 1, "never fewer than one");
}

#[test]
fn a_load_above_the_cores_holds_the_second_check_with_the_sentence() {
    let load = Arc::new(FakeLoad::default());
    load.set(41.4, 16);
    let line = line(&load, 2);
    let (a, b) = (id(), id());
    let first = Check::enter(&line, a, CheckKind::HandIn);
    assert!(first.has_started(), "the first check is never held by load");
    let second = Check::enter(&line, b, CheckKind::HandIn);
    assert!(second.is_waiting(), "load 41 on 16 cores: the second waits");
    let held = LoadHold {
        load: 41,
        cores: 16,
    };
    assert_eq!(line.standing().place(b), Some((1, Some(held))));
    assert_eq!(held.words(), "waits for the machine: load 41 on 16 cores");
    load.set(3.0, 16);
    assert!(second.has_started(), "the load fell: the second starts");
    assert_eq!(line.standing().place(b), Some((0, None)));
    first.finish();
    second.finish();
}

#[test]
fn a_load_equal_to_the_cores_holds_and_one_below_lets_go() {
    let load = Arc::new(FakeLoad::default());
    load.set(16.0, 16);
    let line = line(&load, 2);
    let first = Check::enter(&line, id(), CheckKind::HandIn);
    assert!(first.has_started());
    let second = Check::enter(&line, id(), CheckKind::HandIn);
    assert!(second.is_waiting(), "load 16 on 16 cores is not below");
    load.set(15.9, 16);
    assert!(second.has_started(), "15.9 on 16 cores is below");
    first.finish();
    second.finish();
}

#[test]
fn a_host_without_a_load_number_gates_by_count_alone() {
    let line = Arc::new(CheckLine::default().with_poll(POLL));
    line.set_max_checks(2);
    let first = Check::enter(&line, id(), CheckKind::HandIn);
    let second = Check::enter(&line, id(), CheckKind::HandIn);
    assert!(first.has_started() && second.has_started());
    first.finish();
    second.finish();
}

#[test]
fn an_accepts_recheck_goes_ahead_of_hand_in_checks() {
    let load = Arc::new(FakeLoad::default());
    load.set(0.1, 8);
    let line = line(&load, 1);
    let (running, b, c, d, e) = (id(), id(), id(), id(), id());
    let first = line.enter(running, CheckKind::HandIn);
    for (t, kind) in [
        (b, CheckKind::HandIn),
        (c, CheckKind::HandIn),
        (d, CheckKind::Recheck),
        (e, CheckKind::Recheck),
    ] {
        line.join(t, kind);
    }
    let stand = line.standing();
    assert_eq!(
        [b, c, d, e].map(|t| stand.place(t).map(|(n, _)| n)),
        [Some(3), Some(4), Some(1), Some(2)],
        "the re-checks d, e come first, in the order they joined; then the hand-ins b, c"
    );
    let order = Arc::new(Mutex::new(Vec::new()));
    let threads: Vec<_> = [d, e, b, c]
        .into_iter()
        .map(|t| {
            let (line, order) = (line.clone(), order.clone());
            std::thread::spawn(move || {
                let turn = line.enter(t, CheckKind::HandIn);
                order.lock().expect("order").push(t);
                drop(turn);
            })
        })
        .collect();
    drop(first);
    for t in threads {
        t.join().expect("a check ran");
    }
    assert_eq!(*order.lock().expect("order"), [d, e, b, c]);
}

#[test]
fn between_two_hand_ins_the_first_to_join_goes_first() {
    let load = Arc::new(FakeLoad::default());
    let line = line(&load, 1);
    let (a, b, c) = (id(), id(), id());
    let first = line.enter(a, CheckKind::HandIn);
    line.join(b, CheckKind::HandIn);
    line.join(c, CheckKind::HandIn);
    line.join(b, CheckKind::Recheck);
    assert_eq!(
        line.snapshot(),
        [a, b, c],
        "a join of a card in line changes nothing"
    );
    line.leave(b);
    assert_eq!(line.snapshot(), [a, c]);
    drop(first);
}
