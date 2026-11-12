//! UTC calendar arithmetic on plain numbers: transcripts carry RFC 3339 times and the usage
//! series is per UTC day. No clock is read.

use crate::Timestamp;

const DAY_MS: i64 = 86_400_000;

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number counted from 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The UTC day number (days since 1970-01-01) a time falls on.
pub fn day_of(at: Timestamp) -> i64 {
    at.0.div_euclid(DAY_MS)
}

/// The day number a time falls on for a person whose clock is `offset_minutes` east of UTC
/// (negative to the west): the day of their wall clock, not UTC's.
pub fn day_of_local(at: Timestamp, offset_minutes: i32) -> i64 {
    (at.0 + i64::from(offset_minutes) * 60_000).div_euclid(DAY_MS)
}

/// `YYYY-MM-DD` of a day number.
pub fn day_label(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

fn num(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Reads an RFC 3339 time with a `Z` or numeric offset, such as `2026-11-05T19:57:16.632Z`.
pub fn parse_rfc3339(s: &str) -> Option<Timestamp> {
    let (date, rest) = s.split_once('T')?;
    let mut d = date.split('-');
    let (y, mo, da) = (num(d.next()?)?, num(d.next()?)?, num(d.next()?)?);
    if d.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    let (clock, offset_ms) = if let Some(c) = rest.strip_suffix('Z') {
        (c, 0)
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (c, off) = rest.split_at(at);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let (oh, om) = off[1..].split_once(':')?;
        (c, sign * (num(oh)? * 3_600_000 + num(om)? * 60_000))
    };
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    let mut t = hms.split(':');
    let (h, mi, se) = (num(t.next()?)?, num(t.next()?)?, num(t.next()?)?);
    if t.next().is_some() || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let millis = if frac.is_empty() {
        0
    } else {
        num(&format!("{:0<3}", frac.get(..frac.len().min(3))?))?
    };
    let ms = days_from_civil(y, mo, da) * DAY_MS + ((h * 60 + mi) * 60 + se) * 1000 + millis;
    Some(Timestamp(ms - offset_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_times_become_epoch_milliseconds() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(Timestamp(0)));
        assert_eq!(
            parse_rfc3339("2026-11-05T19:57:16.632Z"),
            Some(Timestamp(1_793_908_636_632))
        );
        assert_eq!(
            parse_rfc3339("2026-11-05T21:57:16.5+02:00"),
            Some(Timestamp(1_793_908_636_500))
        );
        assert_eq!(parse_rfc3339("yesterday"), None);
        assert_eq!(parse_rfc3339("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn days_round_trip_to_labels() {
        let at = parse_rfc3339("2024-02-29T23:59:59Z").expect("time");
        assert_eq!(day_label(day_of(at)), "2024-02-29");
        assert_eq!(day_label(day_of(Timestamp(-1))), "1969-12-31");
        assert_eq!(day_label(day_of(Timestamp(0)) + 1), "1970-01-02");
    }
}
