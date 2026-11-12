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

    /// The instant as `YYYY-MM-DD HH:MM:SS UTC`.
    pub fn utc_text(self) -> String {
        let secs = self.0.div_euclid(1000);
        let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        // Civil date from days since 1970-01-01 (proleptic Gregorian).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }
}
