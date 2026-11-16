//! The line the hand-in checks of a machine stand in. A check runs the project's whole test
//! suite, so the machine admits only `max_checks` at a time (default 1), across projects, and
//! a further one only while the one-minute load average is below the number of cores. The
//! others wait here with their cards in the `Check` turn: an Accept's re-check (a person waits
//! for it) before the hand-in checks, otherwise first come first served. The line lives in
//! memory; after a restart the cards still in the `Check` turn are queued again from the
//! database ([`super::resume`]).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use toknila_core::{LoadHold, TicketId};

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

/// Why a check enters the line: a person waits for a re-check, nobody for a hand-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// The check of a worker's hand-in.
    HandIn,
    /// The check an Accept asked for because main moved: it goes ahead of hand-ins.
    Recheck,
}

/// Where cards stand at one moment: those that run, and those that wait, the next one first.
#[derive(Debug, Clone, Default)]
pub struct Standing {
    running: Vec<TicketId>,
    waiting: Vec<(TicketId, Option<LoadHold>)>,
}

impl Standing {
    /// `Some((0, None))` the card's check runs now, `Some((n, held))` it is the n-th that waits
    /// (1 is next; `held` when the machine's load keeps it from starting), `None` not in line.
    pub fn place(&self, ticket: TicketId) -> Option<(u32, Option<LoadHold>)> {
        if self.running.contains(&ticket) {
            return Some((0, None));
        }
        let at = self.waiting.iter().position(|(t, _)| *t == ticket)?;
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
    running: Vec<TicketId>,
    waiting: VecDeque<(TicketId, CheckKind)>,
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
    ticket: TicketId,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        let mut line = self.line.lock();
        line.running.retain(|t| *t != self.ticket);
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
        Self::queue(&mut line, ticket, kind);
    }

    /// Puts a card that is not in the line yet in its place: a re-check behind the re-checks
    /// already waiting and ahead of every hand-in, a hand-in at the end.
    fn queue(line: &mut Line, ticket: TicketId, kind: CheckKind) {
        if line.running.contains(&ticket) || line.waiting.iter().any(|(t, _)| *t == ticket) {
            return;
        }
        let at = match kind {
            CheckKind::HandIn => line.waiting.len(),
            CheckKind::Recheck => line
                .waiting
                .iter()
                .take_while(|(_, k)| *k == CheckKind::Recheck)
                .count(),
        };
        line.waiting.insert(at, (ticket, kind));
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
        line.waiting.retain(|(t, _)| *t != ticket);
        self.turn.notify_all();
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

    /// Every card in the line, the running ones first.
    pub fn snapshot(&self) -> Vec<TicketId> {
        let line = self.lock();
        let waiting = line.waiting.iter().map(|(t, _)| t);
        line.running.iter().chain(waiting).copied().collect()
    }
}

impl CheckGate for CheckLine {
    fn enter(&self, ticket: TicketId, kind: CheckKind) -> Box<dyn CheckTurn + '_> {
        let mut line = self.lock();
        Self::queue(&mut line, ticket, kind);
        // Only the next card looks at the machine; the others wait behind it. A card held by
        // load looks again every `poll`, since the load changes with no one to wake it.
        while line.waiting.front().map(|(t, _)| t) != Some(&ticket) || !self.admits(&mut line) {
            line = self
                .turn
                .wait_timeout(line, self.poll)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        line.waiting.pop_front();
        line.held = None;
        line.running.push(ticket);
        Box::new(Turn { line: self, ticket })
    }
}
