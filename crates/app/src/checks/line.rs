//! The line the checks of a machine stand in. A check runs the project's whole test suite, so
//! the machine admits only `max_checks` at a time (default 1), across projects, and a further one
//! only while the one-minute load average is below the number of cores. The others wait here:
//! an Accept's re-check and a release's Check (a person waits for them) before the hand-in
//! checks, otherwise first come first served. The line lives in memory; after a restart the
//! cards still in the `Check` turn are queued again from the database ([`super::resume`]).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use toknila_core::{LoadHold, ReleaseId, TicketId};

use super::{CheckGate, CheckTurn};

/// The machine's load as the host read it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Load {
    /// The one-minute load average.
    pub one_minute: f64,
    /// The number of CPU cores.
    pub cores: u32,
}

/// Where the line reads the machine's load from; `None` when the host has no such number (then
/// there is no load gate).
pub trait LoadReader: std::fmt::Debug + Send + Sync {
    /// The load now.
    fn read(&self) -> Option<Load>;
}

/// A host without a load number: checks are admitted by count alone.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLoad;

impl LoadReader for NoLoad {
    fn read(&self) -> Option<Load> {
        None
    }
}

/// Why a check enters the line: a person waits for a re-check or a release, nobody for a hand-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// The check of a worker's hand-in.
    HandIn,
    /// The check an Accept asked for because main moved: it goes ahead of hand-ins.
    Recheck,
    /// A release's Check step: it goes ahead of hand-ins, in order with the re-checks.
    Release,
}

/// Whose check stands in the line: a card's, or a release run's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Card(TicketId),
    Release(ReleaseId),
}

/// Where checks stand at one moment: those that run, and those that wait, the next one first.
#[derive(Debug, Clone, Default)]
pub struct Standing {
    running: Vec<Who>,
    waiting: Vec<(Who, Option<LoadHold>)>,
}

impl Standing {
    /// `Some((0, None))` the card's check runs now, `Some((n, held))` it is the n-th that waits
    /// (1 is next; `held` when the machine's load keeps it from starting), `None` not in line.
    pub fn place(&self, ticket: TicketId) -> Option<(u32, Option<LoadHold>)> {
        self.place_of(Who::Card(ticket))
    }

    /// [`Standing::place`] of a release run's Check.
    pub fn place_release(&self, release: ReleaseId) -> Option<(u32, Option<LoadHold>)> {
        self.place_of(Who::Release(release))
    }

    fn place_of(&self, who: Who) -> Option<(u32, Option<LoadHold>)> {
        if self.running.contains(&who) {
            return Some((0, None));
        }
        let at = self.waiting.iter().position(|(w, _)| *w == who)?;
        Some((u32::try_from(at + 1).ok()?, self.waiting[at].1))
    }
}

/// A load average as the whole number the card shows.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole(load: f64) -> u32 {
    // Saturating: a float outside u32 clamps, a NaN is 0.
    load.round().max(0.0) as u32
}

#[derive(Debug, Default)]
struct Line {
    running: Vec<Who>,
    waiting: VecDeque<(Who, CheckKind)>,
    held: Option<LoadHold>,
}

/// The line of checks of one machine.
#[derive(Debug)]
pub struct CheckLine {
    line: Mutex<Line>,
    turn: Condvar,
    max: AtomicU32,
    load: Arc<dyn LoadReader>,
    poll: Duration,
}

impl Default for CheckLine {
    fn default() -> Self {
        Self::new(Arc::new(NoLoad))
    }
}

/// The turn of a check; the next card in the line starts when it is dropped.
#[derive(Debug)]
pub struct Turn<'a> {
    line: &'a CheckLine,
    who: Who,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        let mut line = self.line.lock();
        line.running.retain(|w| *w != self.who);
        self.line.turn.notify_all();
    }
}

impl CheckLine {
    /// A line that reads the machine's load from `load`.
    pub fn new(load: Arc<dyn LoadReader>) -> Self {
        Self {
            line: Mutex::default(),
            turn: Condvar::new(),
            max: AtomicU32::new(1),
            load,
            poll: Duration::from_secs(5),
        }
    }

    /// How often a card held back by load looks at the load again.
    pub fn with_poll(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }

    fn lock(&self) -> MutexGuard<'_, Line> {
        self.line
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// How many checks may run at once.
    pub fn max_checks(&self) -> u32 {
        self.max.load(Ordering::Relaxed)
    }

    /// Sets how many checks may run at once (at least 1); waiting cards look again at once.
    pub fn set_max_checks(&self, n: u32) {
        self.max.store(n.max(1), Ordering::Relaxed);
        self.turn.notify_all();
    }

    /// The card takes its place in the line, so it shows as waiting before its check is picked
    /// up. Does nothing when it is already in the line.
    pub fn join(&self, ticket: TicketId, kind: CheckKind) {
        let mut line = self.lock();
        Self::queue(&mut line, Who::Card(ticket), kind);
    }

    /// A release run's Check takes its place in the line ([`CheckKind::Release`]).
    pub fn join_release(&self, release: ReleaseId) {
        let mut line = self.lock();
        Self::queue(&mut line, Who::Release(release), CheckKind::Release);
    }

    /// Puts a check that is not in the line yet in its place: a re-check or a release's Check
    /// behind those of them already waiting and ahead of every hand-in, a hand-in at the end.
    fn queue(line: &mut Line, who: Who, kind: CheckKind) {
        if line.running.contains(&who) || line.waiting.iter().any(|(w, _)| *w == who) {
            return;
        }
        let at = match kind {
            CheckKind::HandIn => line.waiting.len(),
            CheckKind::Recheck | CheckKind::Release => line
                .waiting
                .iter()
                .take_while(|(_, k)| *k != CheckKind::HandIn)
                .count(),
        };
        line.waiting.insert(at, (who, kind));
    }

    /// Whether the next card may start now: fewer than `max_checks` run and, when another check
    /// already runs, the load is below the cores. Records why the card holds back for the load.
    fn admits(&self, line: &mut Line) -> bool {
        let running = line.running.len();
        if running >= usize::try_from(self.max_checks()).unwrap_or(usize::MAX) {
            line.held = None;
            return false;
        }
        // The first check always starts: the load is only a reason to wait for a second one.
        let load = self.load.read().filter(|_| running > 0);
        match load {
            Some(l) if l.one_minute >= f64::from(l.cores) => {
                line.held = Some(LoadHold {
                    load: whole(l.one_minute),
                    cores: l.cores,
                });
                false
            }
            _ => {
                line.held = None;
                true
            }
        }
    }

    /// The card leaves the line without having run (its work was dropped).
    pub fn leave(&self, ticket: TicketId) {
        let mut line = self.lock();
        line.waiting.retain(|(w, _)| *w != Who::Card(ticket));
        self.turn.notify_all();
    }

    /// A release run's Check joins the line (when it is not in it) and waits for its turn,
    /// which lasts until the returned guard is dropped.
    pub fn enter_release(&self, release: ReleaseId) -> Turn<'_> {
        self.enter_as(Who::Release(release), CheckKind::Release)
    }

    fn enter_as(&self, who: Who, kind: CheckKind) -> Turn<'_> {
        let mut line = self.lock();
        Self::queue(&mut line, who, kind);
        // Only the next check looks at the machine; the others wait behind it. One held by load
        // looks again every `poll`, since the load changes with no one to wake it.
        while line.waiting.front().map(|(w, _)| w) != Some(&who) || !self.admits(&mut line) {
            line = self
                .turn
                .wait_timeout(line, self.poll)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        line.waiting.pop_front();
        line.held = None;
        line.running.push(who);
        Turn { line: self, who }
    }

    /// Where every card in the line stands.
    pub fn standing(&self) -> Standing {
        let line = self.lock();
        Standing {
            running: line.running.clone(),
            waiting: line
                .waiting
                .iter()
                .enumerate()
                .map(|(i, (t, _))| (*t, if i == 0 { line.held } else { None }))
                .collect(),
        }
    }

    /// Every card in the line, the running ones first (a release's Check is left out).
    pub fn snapshot(&self) -> Vec<TicketId> {
        let line = self.lock();
        let waiting = line.waiting.iter().map(|(w, _)| w);
        let card = |w: &Who| match w {
            Who::Card(t) => Some(*t),
            Who::Release(_) => None,
        };
        line.running
            .iter()
            .chain(waiting)
            .filter_map(card)
            .collect()
    }
}

impl CheckGate for CheckLine {
    fn enter(&self, ticket: TicketId, kind: CheckKind) -> Box<dyn CheckTurn + '_> {
        Box::new(self.enter_as(Who::Card(ticket), kind))
    }
}
