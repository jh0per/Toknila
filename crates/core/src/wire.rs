//! Names of values as they appear on the wire: the serde name of a unit enum variant
//! (`ReleaseChannel::TagOnly` -> `tag_only`). One definition for the database, chain files
//! and the comment template, so the three cannot drift.

use serde::Serialize;

/// The serde name of a unit enum variant; `None` for anything that is not a plain string.
pub fn wire_name<T: Serialize>(value: &T) -> Option<String> {
    toml::Value::try_from(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommentKind, ReleaseChannel, TicketStatus, WaitingReason};

    #[test]
    fn unit_variants_use_their_serde_names() {
        assert_eq!(
            wire_name(&ReleaseChannel::TagOnly).as_deref(),
            Some("tag_only")
        );
        assert_eq!(
            wire_name(&TicketStatus::Running).as_deref(),
            Some("running")
        );
        assert_eq!(
            wire_name(&CommentKind::Decision).as_deref(),
            Some("decision")
        );
    }

    #[test]
    fn non_strings_have_no_wire_name() {
        assert_eq!(wire_name(&WaitingReason::Silent), None);
        assert_eq!(wire_name(&3_u32), None);
    }
}
