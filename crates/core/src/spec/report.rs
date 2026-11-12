//! What an import tells about cards that were done out of order.

use std::fmt;

use super::{SpecPhase, task_key};

/// A card whose tasks are all checked although cards before it still have open ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutOfOrderCard {
    /// `phase-3`, or the title of a card without a phase number.
    pub card: String,
    /// The earlier cards that still have open tasks, in file order.
    pub behind: Vec<OpenBefore>,
}

/// An earlier card and the keys of its open tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenBefore {
    pub card: String,
    pub open: Vec<String>,
}

impl fmt::Display for OutOfOrderCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} done out of order: ", self.card)?;
        for (i, b) in self.behind.iter().enumerate() {
            let s = if b.open.len() == 1 { "" } else { "s" };
            let sep = if i == 0 { "" } else { "; " };
            let keys = b.open.join(", ");
            write!(
                f,
                "{sep}{} has {} open task{s} ({keys})",
                b.card,
                b.open.len()
            )?;
        }
        Ok(())
    }
}

/// How a card is named in the report: its phase number, else its title.
pub(super) fn label(phase: &SpecPhase, title: &str) -> String {
    match phase.number {
        Some(n) => format!("phase-{n}"),
        None => title.to_owned(),
    }
}

/// The report for `cards` (phase, resolved title) given, for each, the earlier cards still open.
pub(super) fn build(
    cards: &[(&SpecPhase, &str)],
    open_before: &[Vec<usize>],
) -> Vec<OutOfOrderCard> {
    cards
        .iter()
        .zip(open_before)
        .filter(|(_, open)| !open.is_empty())
        .map(|((phase, title), open)| OutOfOrderCard {
            card: label(phase, title),
            behind: open
                .iter()
                .map(|i| {
                    let (p, t) = cards[*i];
                    OpenBefore {
                        card: label(p, t),
                        open: p
                            .tasks
                            .iter()
                            .filter(|t| !t.done)
                            .map(|t| task_key(&t.text).to_owned())
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect()
}
