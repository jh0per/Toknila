//! Moving a card to another epic of its project: who may, and where it may go. Pure rules; the
//! application moves the rows.

use crate::{Chain, Ticket, TicketKey, TicketStatus};

/// Why a card may not move to another epic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CardMoveError {
    /// The card is on its way or done: it has a ship, a branch or a history under its epic.
    #[error(
        "{card} is {status}; only a story that has not started (backlog or ready) or a cancelled \
         one can move to another epic"
    )]
    Started { card: String, status: String },
    /// The card has a ship branch or a ship even though its status looks unstarted.
    #[error("{card} has a ship ({branch}); a story with a history under its epic stays in it")]
    HasShip { card: String, branch: String },
    /// The target is the epic the card is in.
    #[error("{card} is in epic {chain} already")]
    SameChain { card: String, chain: String },
    /// The target epic belongs to another project.
    #[error("epic {chain} belongs to another project; a story moves between epics of its own")]
    OtherProject { chain: String },
    /// The target epic starts its ready stories by itself; the moved card could start at once.
    #[error(
        "epic {chain} starts its ready stories by itself (autostart), so the moved story could \
         start at once. Turn autostart off first"
    )]
    AutostartTarget { chain: String },
    /// The card waits for cards of its epic, or cards wait for it.
    #[error(
        "{card} depends on or is depended on by {linked} in its epic; remove those dependencies \
         first, then move it"
    )]
    Linked { card: String, linked: String },
}

/// Whether the card can move to another epic at all: it has not started (or is cancelled) and has
/// no ship. Which epic it may go to is [`check`]'s.
pub fn may_leave(card: &Ticket, has_ship: bool) -> bool {
    matches!(
        card.status,
        TicketStatus::Backlog | TicketStatus::Ready | TicketStatus::Cancelled
    ) && !has_ship
        && card.branch.is_none()
}

/// Whether `card` may leave `from` for `to`, given the cards it is linked to by a dependency.
pub fn check(
    card: &Ticket,
    from: &Chain,
    to: &Chain,
    has_ship: bool,
    linked: &[TicketKey],
) -> Result<(), CardMoveError> {
    if from.project_id != to.project_id {
        return Err(CardMoveError::OtherProject {
            chain: to.name.clone(),
        });
    }
    if from.id == to.id {
        return Err(CardMoveError::SameChain {
            card: card.key.to_string(),
            chain: to.name.clone(),
        });
    }
    if !matches!(
        card.status,
        TicketStatus::Backlog | TicketStatus::Ready | TicketStatus::Cancelled
    ) {
        return Err(CardMoveError::Started {
            card: card.key.to_string(),
            status: crate::wire::wire_name(&card.status).unwrap_or_default(),
        });
    }
    if has_ship || card.branch.is_some() {
        return Err(CardMoveError::HasShip {
            card: card.key.to_string(),
            branch: card
                .branch
                .clone()
                .unwrap_or_else(|| "no branch".to_owned()),
        });
    }
    if to.autostart {
        return Err(CardMoveError::AutostartTarget {
            chain: to.name.clone(),
        });
    }
    if !linked.is_empty() {
        let linked = linked
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CardMoveError::Linked {
            card: card.key.to_string(),
            linked,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, ProjectId, TicketDraft, TicketId, Timestamp};

    fn chain(project: ProjectId, name: &str, autostart: bool) -> Chain {
        Chain {
            id: ChainId::new(),
            project_id: project,
            name: name.to_owned(),
            title: None,
            goal: String::new(),
            autostart,
            review_mode: None,
            reviewer: None,
            source: None,
            position: 0,
            version: None,
        }
    }

    fn card(chain: &Chain, status: TicketStatus) -> Ticket {
        let key = "DEMO-1".parse().expect("key");
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            chain.id,
            key,
            TicketDraft::default(),
            Timestamp(1),
        );
        t.status = status;
        t
    }

    fn pair() -> (Chain, Chain) {
        let p = ProjectId::new();
        (chain(p, "soundpacks", false), chain(p, "android", false))
    }

    #[test]
    fn a_story_that_has_not_started_and_a_cancelled_one_may_move() {
        let (a, b) = pair();
        for s in [
            TicketStatus::Backlog,
            TicketStatus::Ready,
            TicketStatus::Cancelled,
        ] {
            assert_eq!(check(&card(&a, s), &a, &b, false, &[]), Ok(()), "{s:?}");
        }
    }

    #[test]
    fn a_story_on_its_way_or_done_stays() {
        let (a, b) = pair();
        for s in [
            TicketStatus::Running,
            TicketStatus::Waiting,
            TicketStatus::Blocked,
            TicketStatus::Review,
            TicketStatus::Merged,
            TicketStatus::Released,
            TicketStatus::Proposed,
        ] {
            let e = check(&card(&a, s), &a, &b, false, &[]).expect_err("refused");
            assert!(matches!(e, CardMoveError::Started { .. }), "{s:?}: {e}");
        }
    }

    #[test]
    fn a_ship_or_a_branch_keeps_the_story_where_it_is() {
        let (a, b) = pair();
        let mut t = card(&a, TicketStatus::Cancelled);
        assert!(matches!(
            check(&t, &a, &b, true, &[]),
            Err(CardMoveError::HasShip { .. })
        ));
        t.branch = Some("ship/soundpacks/x".to_owned());
        let e = check(&t, &a, &b, false, &[]).expect_err("branch");
        assert!(e.to_string().contains("ship/soundpacks/x"), "{e}");
    }

    #[test]
    fn the_target_must_be_another_epic_of_the_same_project_that_waits_for_the_owner() {
        let (a, b) = pair();
        let t = card(&a, TicketStatus::Ready);
        assert!(matches!(
            check(&t, &a, &a, false, &[]),
            Err(CardMoveError::SameChain { .. })
        ));
        let other = chain(ProjectId::new(), "android", false);
        assert!(matches!(
            check(&t, &a, &other, false, &[]),
            Err(CardMoveError::OtherProject { .. })
        ));
        let auto = chain(a.project_id, "auto", true);
        let e = check(&t, &a, &auto, false, &[]).expect_err("autostart");
        assert!(matches!(e, CardMoveError::AutostartTarget { .. }), "{e}");
        assert_eq!(check(&t, &a, &b, false, &[]), Ok(()));
    }

    #[test]
    fn a_dependency_on_a_card_of_its_epic_is_named() {
        let (a, b) = pair();
        let t = card(&a, TicketStatus::Backlog);
        let linked: Vec<TicketKey> = ["DEMO-2", "DEMO-3"]
            .iter()
            .map(|k| k.parse().expect("key"))
            .collect();
        let e = check(&t, &a, &b, false, &linked).expect_err("linked");
        assert!(e.to_string().contains("DEMO-2, DEMO-3"), "{e}");
    }
}
