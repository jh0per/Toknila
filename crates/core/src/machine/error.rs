use crate::{Actor, TicketKey, TicketStatus, thread::OpenThread};

/// Why an event was refused. The ticket is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("cannot {event} a ticket in {from:?}")]
    NotAllowed {
        event: &'static str,
        from: TicketStatus,
    },
    #[error(
        "cannot {event} yet: the card is still being checked (brought up to main and the check command run); Return or Accept once it waits for a person"
    )]
    BeingChecked { event: &'static str },
    #[error("ticket is paused; resume it first")]
    Paused,
    #[error("ticket is already started and waits for a free slot")]
    AlreadyQueued,
    #[error("ticket is not paused")]
    NotPaused,
    #[error("{0} needs a text")]
    EmptyText(&'static str),
    #[error("cannot hand in {0}: {open}", open = crate::thread::list(.1))]
    OpenThreads(TicketKey, Vec<OpenThread>),
    #[error(
        "cannot hand in {0}: it was asked to show a design first, and no design was approved. \
         Open a design thread with `open_thread` (kind design), post a first version with \
         `thread_version`, and wait for the person to approve it; then call `finished` again"
    )]
    DesignNotApproved(TicketKey),
    #[error("a card is done out of order only when a card before it is still open")]
    NothingBehind,
    #[error(transparent)]
    Items(#[from] crate::ItemsError),
    #[error("only a person can {event} a card; this came from {actor:?}")]
    PersonOnly { event: &'static str, actor: Actor },
}
