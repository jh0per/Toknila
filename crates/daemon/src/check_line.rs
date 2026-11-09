//! The line the hand-in checks of this machine stand in. A check runs the project's whole test
//! suite, so only one runs at a time, across projects: the others wait here, in the order they
//! were handed in, with their cards in the `Check` turn. The line lives in memory; after a
//! restart the cards still in the `Check` turn are queued again from the database
//! ([`toknila_app::checks::resume`]).

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, MutexGuard};

use toknila_app::checks::{CheckGate, CheckTurn};
use toknila_core::TicketId;

#[derive(Debug, Default)]
struct Line {
    /// The card whose check runs now.
    running: Option<TicketId>,
    /// The cards that wait, the next one first.
    waiting: VecDeque<TicketId>,
}

/// The line of checks of one daemon.
#[derive(Debug, Default)]
pub struct CheckLine {
    line: Mutex<Line>,
    turn: Condvar,
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
        if line.running == Some(self.ticket) {
            line.running = None;
        }
        self.line.turn.notify_all();
    }
}

impl CheckLine {
    fn lock(&self) -> MutexGuard<'_, Line> {
        self.line
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The card takes its place at the end of the line, so it shows as waiting before its
    /// check is picked up. Does nothing when it is already in the line.
    pub fn join(&self, ticket: TicketId) {
        let mut line = self.lock();
        if line.running != Some(ticket) && !line.waiting.contains(&ticket) {
            line.waiting.push_back(ticket);
        }
    }

    /// The card leaves the line without having run (its work was dropped).
    pub fn leave(&self, ticket: TicketId) {
        let mut line = self.lock();
        line.waiting.retain(|t| *t != ticket);
        self.turn.notify_all();
    }

    /// Where `ticket` stands: `Some(0)` its check runs now, `Some(n)` it is the n-th that waits
    /// (1 is next), `None` it is not in the line.
    pub fn position(&self, ticket: TicketId) -> Option<u32> {
        let line = self.lock();
        if line.running == Some(ticket) {
            return Some(0);
        }
        let at = line.waiting.iter().position(|t| *t == ticket)?;
        u32::try_from(at + 1).ok()
    }

    /// Every card in the line, the running one first.
    pub fn snapshot(&self) -> Vec<TicketId> {
        let line = self.lock();
        line.running.iter().chain(&line.waiting).copied().collect()
    }
}

impl CheckGate for CheckLine {
    fn enter(&self, ticket: TicketId) -> Box<dyn CheckTurn + '_> {
        let mut line = self.lock();
        if line.running != Some(ticket) && !line.waiting.contains(&ticket) {
            line.waiting.push_back(ticket);
        }
        while line.running.is_some() || line.waiting.front() != Some(&ticket) {
            line = self
                .turn
                .wait(line)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        line.waiting.pop_front();
        line.running = Some(ticket);
        Box::new(Turn { line: self, ticket })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn id() -> TicketId {
        TicketId::new()
    }

    #[test]
    fn the_second_card_waits_until_the_first_turn_is_dropped() {
        let line = Arc::new(CheckLine::default());
        let (a, b) = (id(), id());
        let first = line.enter(a);
        line.join(b);
        assert_eq!(
            (line.position(a), line.position(b)),
            (Some(0), Some(1)),
            "a runs, b is next"
        );
        let waiter = std::thread::spawn({
            let line = line.clone();
            move || drop(line.enter(b))
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiter.is_finished(), "b still waits while a runs");
        drop(first);
        waiter.join().expect("b ran after a");
        assert_eq!(line.snapshot(), Vec::<TicketId>::new());
    }

    #[test]
    fn the_line_keeps_the_order_cards_joined_in() {
        let line = CheckLine::default();
        let (a, b, c) = (id(), id(), id());
        for t in [a, b, c, b] {
            line.join(t);
        }
        assert_eq!(line.snapshot(), [a, b, c]);
        line.leave(b);
        assert_eq!(line.snapshot(), [a, c]);
        assert_eq!(line.position(c), Some(2));
    }
}
