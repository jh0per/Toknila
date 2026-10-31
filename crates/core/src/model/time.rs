//! Time as plain data. Core never reads the clock; callers pass the current time in, which keeps
//! every rule deterministic under test.

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch, UTC.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Milliseconds elapsed from `earlier` to `self`, saturating at zero.
    pub fn millis_since(self, earlier: Timestamp) -> u64 {
        u64::try_from(self.0.saturating_sub(earlier.0)).unwrap_or(0)
    }
}
