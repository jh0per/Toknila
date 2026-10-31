//! Typed ULID identifiers. Each entity gets its own type so a `TicketId` can never be passed
//! where a `ChainId` is expected.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Ulid);

        impl $name {
            /// A fresh, time-ordered identifier. Reads the clock and OS randomness, so rules
            /// in core never call it; the daemon mints ids and passes them in.
            pub fn new() -> Self {
                Self(Ulid::generate())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = ulid::DecodeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ulid::from_string(s).map(Self)
            }
        }
    };
}

id_type!(
    /// Identifies a [`crate::Project`].
    ProjectId
);
id_type!(
    /// Identifies a [`crate::Chain`].
    ChainId
);
id_type!(
    /// Identifies a [`crate::Ticket`].
    TicketId
);
id_type!(
    /// Identifies a [`crate::Subtask`].
    SubtaskId
);
id_type!(
    /// Identifies a [`crate::Comment`].
    CommentId
);
id_type!(
    /// Identifies a [`crate::Release`] milestone.
    ReleaseId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_display_parses_back() {
        let id = TicketId::new();
        assert_eq!(id.to_string().parse::<TicketId>().ok(), Some(id));
    }

    #[test]
    fn id_serializes_as_bare_string() {
        let id: ChainId = "01JBQ7ZJ0000000000000000AB".parse().expect("valid ulid");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, "\"01JBQ7ZJ0000000000000000AB\"");
    }
}
