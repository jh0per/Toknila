//! The merge line of a project. Accepted cards merge one at a time, in the order they joined the
//! line, each holding its turn across "sync with main, re-check when main moved, merge", so its
//! re-check runs against a main that stays. A release run holds the same line from its start
//! until its Tag (and Push) ended: it waits for the turn in progress and goes ahead of the cards
//! that wait, so no card merges while it tags. The rules are here; the app gives the turn and the
//! host keeps the release runs' part.

use serde::{Deserialize, Serialize};

use crate::{ReleaseId, ReviewTurn, Ticket, TicketId, TicketKey, TicketStatus, Timestamp};

/// An accepted card's place in its project's merge line, kept on its review.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinePlace {
    /// When the card joined the line; the cards that wait go in this order.
    pub since: Timestamp,
    /// The card holds the turn: its merge (and the re-check it needs) runs, and no other card of
    /// the project merges until it ends.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub holds: bool,
}

/// Whether `t` is in its merge: accepted and merging, or in the re-check its Accept asked for.
pub fn in_merge(t: &Ticket) -> bool {
    t.status == TicketStatus::Review
        && match t.review.turn {
            Some(ReviewTurn::Merging) => true,
            Some(ReviewTurn::Check) => t.review.recheck,
            _ => false,
        }
}

/// Keeps the card's place in step with its state after a transition at `now`: a card that
/// entered its merge joins the line at the end; one that left it (merged, blocked, back to the
/// worker, cancelled, to a person) leaves the line and frees its turn.
pub fn stamp(next: &mut Ticket, now: Timestamp) {
    if !in_merge(next) {
        next.review.merge_line = None;
    } else if next.review.merge_line.is_none() {
        next.review.merge_line = Some(LinePlace {
            since: now,
            holds: false,
        });
    }
}

/// A release run in its project's merge line, as the host keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTurn {
    pub release: ReleaseId,
    pub version: String,
    /// The run took the turn; until then it waits for the turn in progress.
    pub holds: bool,
}

/// One who stands in a merge line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entrant {
    Card { id: TicketId, key: TicketKey },
    Release { id: ReleaseId, version: String },
}

/// What a card or a release waits for in the merge line.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "for", rename_all = "snake_case")]
pub enum MergeWait {
    /// A release run is ahead (it holds the line or takes it next): the card merges after its Tag.
    Release { version: String },
    /// The card `key` holds the turn (it is first); the one that waits is `place`-th in line.
    Card { key: TicketKey, place: u32 },
}

impl MergeWait {
    /// `waits for the release of 0.3.8`, `waits for DEMO-3 to merge (#2 in line)`.
    pub fn words(&self) -> String {
        match self {
            Self::Release { version } => format!("waits for the release of {version}"),
            Self::Card { key, place } => format!("waits for {key} to merge (#{place} in line)"),
        }
    }

    /// What an accepted card says: `accepted; waits for DEMO-3 to merge (#2 in line)`.
    pub fn card_words(&self) -> String {
        format!("accepted; {}", self.words())
    }
}

/// A project's merge line at one moment: who holds the turn, and who waits, the next first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub holder: Option<Entrant>,
    pub waiting: Vec<Entrant>,
}

impl Line {
    /// The line of a project: those of `cards` that are in their merge (any of the project's
    /// tickets may be given), and `releases` in the order their runs began. Releases that wait go
    /// ahead of the cards that wait; cards wait in the order they joined.
    pub fn of(cards: &[Ticket], releases: &[ReleaseTurn]) -> Self {
        let mut placed: Vec<(&Ticket, LinePlace)> = cards
            .iter()
            .filter(|t| in_merge(t))
            .filter_map(|t| t.review.merge_line.map(|p| (t, p)))
            .collect();
        placed.sort_by(|(a, pa), (b, pb)| {
            (pa.since, a.key.to_string()).cmp(&(pb.since, b.key.to_string()))
        });
        let card = |t: &Ticket| Entrant::Card {
            id: t.id,
            key: t.key.clone(),
        };
        let release = |r: &ReleaseTurn| Entrant::Release {
            id: r.release,
            version: r.version.clone(),
        };
        let mut holders: Vec<Entrant> = placed
            .iter()
            .filter(|(_, p)| p.holds)
            .map(|(t, _)| card(t))
            .chain(releases.iter().filter(|r| r.holds).map(release))
            .collect();
        let holder = (!holders.is_empty()).then(|| holders.remove(0));
        // A second holder cannot happen; should it, it is next rather than lost.
        let waiting = holders
            .into_iter()
            .chain(releases.iter().filter(|r| !r.holds).map(release))
            .chain(
                placed
                    .iter()
                    .filter(|(_, p)| !p.holds)
                    .map(|(t, _)| card(t)),
            )
            .collect();
        Self { holder, waiting }
    }

    /// Who takes the turn now: the first that waits, when nobody holds it.
    pub fn next(&self) -> Option<&Entrant> {
        match self.holder {
            Some(_) => None,
            None => self.waiting.first(),
        }
    }

    /// What the card `id` waits for; `None` when it holds the turn or is not in the line.
    pub fn card_wait(&self, id: TicketId) -> Option<MergeWait> {
        let at = self
            .waiting
            .iter()
            .position(|e| matches!(e, Entrant::Card { id: c, .. } if *c == id))?;
        self.wait_at(at)
    }

    /// What the release `id`'s run waits for; `None` when it holds the turn or takes it now.
    pub fn release_wait(&self, id: ReleaseId) -> Option<MergeWait> {
        let at = self
            .waiting
            .iter()
            .position(|e| matches!(e, Entrant::Release { id: r, .. } if *r == id))?;
        self.wait_at(at)
    }

    /// The wait of the one at `at` among those that wait: a release ahead of it first, else the
    /// card that holds the turn (or is next), with its place counted from the holder.
    fn wait_at(&self, at: usize) -> Option<MergeWait> {
        let ahead: Vec<&Entrant> = self
            .holder
            .iter()
            .chain(self.waiting[..at].iter())
            .collect();
        let place = u32::try_from(ahead.len() + 1).unwrap_or(u32::MAX);
        if let Some(Entrant::Release { version, .. }) =
            ahead.iter().find(|e| matches!(e, Entrant::Release { .. }))
        {
            return Some(MergeWait::Release {
                version: version.clone(),
            });
        }
        match ahead.first() {
            Some(Entrant::Card { key, .. }) => Some(MergeWait::Card {
                key: key.clone(),
                place,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, TicketDraft};

    fn card(n: u32, since: i64, holds: bool) -> Ticket {
        let key = format!("DEMO-{n}").parse().expect("key");
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            key,
            TicketDraft::default(),
            Timestamp(1),
        );
        t.status = TicketStatus::Review;
        t.review.turn = Some(ReviewTurn::Merging);
        t.review.merge_line = Some(LinePlace {
            since: Timestamp(since),
            holds,
        });
        t
    }

    fn release(version: &str, holds: bool) -> ReleaseTurn {
        ReleaseTurn {
            release: ReleaseId::new(),
            version: version.to_owned(),
            holds,
        }
    }

    #[test]
    fn a_card_joins_the_line_when_it_enters_its_merge_and_leaves_it_with_its_turn() {
        let mut t = card(1, 5, true);
        t.review.merge_line = None;
        stamp(&mut t, Timestamp(7));
        assert_eq!(
            t.review.merge_line,
            Some(LinePlace {
                since: Timestamp(7),
                holds: false
            })
        );
        t.review.merge_line.as_mut().expect("place").holds = true;
        // The re-check its Accept asked for keeps the place and the turn.
        t.review.turn = Some(ReviewTurn::Check);
        t.review.recheck = true;
        stamp(&mut t, Timestamp(9));
        assert_eq!(
            t.review.merge_line,
            Some(LinePlace {
                since: Timestamp(7),
                holds: true
            })
        );
        // A hand-in's check is not a merge.
        t.review.recheck = false;
        stamp(&mut t, Timestamp(10));
        assert_eq!(t.review.merge_line, None);
        let mut gone = card(2, 1, true);
        gone.status = TicketStatus::Merged;
        stamp(&mut gone, Timestamp(3));
        assert_eq!(gone.review.merge_line, None);
    }

    #[test]
    fn cards_wait_in_the_order_they_joined_and_say_who_holds_the_turn() {
        let (first, second, third) = (card(5, 10, true), card(9, 20, false), card(2, 30, false));
        let mut out = card(7, 1, false);
        out.status = TicketStatus::Running;
        let cards = [third.clone(), out, second.clone(), first.clone()];
        let line = Line::of(&cards, &[]);
        assert!(matches!(&line.holder, Some(Entrant::Card { id, .. }) if *id == first.id));
        assert_eq!(line.waiting.len(), 2, "the running card is not in line");
        assert_eq!(line.next(), None, "the turn is taken");
        let wait = line.card_wait(third.id).expect("waits");
        assert_eq!(
            wait.card_words(),
            "accepted; waits for DEMO-5 to merge (#3 in line)"
        );
        assert_eq!(
            line.card_wait(second.id).map(|w| w.words()).as_deref(),
            Some("waits for DEMO-5 to merge (#2 in line)")
        );
        assert_eq!(line.card_wait(first.id), None, "it holds the turn");
        let free = Line::of(&[third.clone(), second.clone()], &[]);
        assert!(
            matches!(free.next(), Some(Entrant::Card { id, .. }) if *id == second.id),
            "the one that joined first is next"
        );
    }

    #[test]
    fn a_release_waits_for_the_turn_in_progress_and_goes_ahead_of_the_waiting_cards() {
        let (holder, waiting) = (card(1, 10, true), card(2, 20, false));
        let r = release("0.3.8", false);
        let line = Line::of(&[waiting.clone(), holder.clone()], std::slice::from_ref(&r));
        assert_eq!(
            line.release_wait(r.release).map(|w| w.words()).as_deref(),
            Some("waits for DEMO-1 to merge (#2 in line)")
        );
        assert_eq!(
            line.card_wait(waiting.id)
                .map(|w| w.card_words())
                .as_deref(),
            Some("accepted; waits for the release of 0.3.8")
        );
        let free = Line::of(std::slice::from_ref(&waiting), std::slice::from_ref(&r));
        assert!(
            matches!(free.next(), Some(Entrant::Release { id, .. }) if *id == r.release),
            "with the turn free the release takes it before the card"
        );
        let held = Line::of(std::slice::from_ref(&waiting), &[release("0.3.8", true)]);
        assert!(matches!(held.holder, Some(Entrant::Release { .. })));
        assert_eq!(
            held.card_wait(waiting.id)
                .map(|w| w.card_words())
                .as_deref(),
            Some("accepted; waits for the release of 0.3.8")
        );
    }

    #[test]
    fn the_wait_is_pinned_on_the_wire() {
        let key: TicketKey = "DEMO-3".parse().expect("key");
        let json = serde_json::to_string(&MergeWait::Card { key, place: 2 }).expect("json");
        assert_eq!(json, r#"{"for":"card","key":"DEMO-3","place":2}"#);
        let json = serde_json::to_string(&MergeWait::Release {
            version: "0.3.8".to_owned(),
        })
        .expect("json");
        assert_eq!(json, r#"{"for":"release","version":"0.3.8"}"#);
        let place = LinePlace {
            since: Timestamp(4),
            holds: false,
        };
        assert_eq!(
            serde_json::to_string(&place).expect("json"),
            r#"{"since":4}"#
        );
    }
}
