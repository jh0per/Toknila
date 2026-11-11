//! How `tok ls` and `tok show` word a card in the Check turn.

use toknila_core::CheckPhase;

/// `being checked: cargo xtask verify, 3 min`, `being checked: syncing with main` or
/// `being checked: queued, next in line`; `now_ms` is the clock the elapsed time is read against.
pub fn check_words(phase: Option<&CheckPhase>, now_ms: i64) -> String {
    let what = match phase {
        None => "starting".to_owned(),
        Some(CheckPhase::Queued { place: 1 }) => "queued, next in line".to_owned(),
        Some(CheckPhase::Queued { place }) => format!("queued, #{place} in line"),
        Some(CheckPhase::Syncing) => "syncing with main".to_owned(),
        Some(CheckPhase::Running { command, since }) => {
            let secs = (now_ms - since.0).max(0) / 1000;
            let elapsed = if secs < 60 {
                format!("{secs} s")
            } else {
                format!("{} min", secs / 60)
            };
            format!("{command}, {elapsed}")
        }
    };
    format!("being checked: {what}")
}

/// [`check_words`] read against the wall clock.
pub fn check_words_now(phase: Option<&CheckPhase>) -> String {
    check_words(phase, chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::Timestamp;

    #[test]
    fn each_phase_says_where_the_check_is() {
        let running = CheckPhase::Running {
            command: "cargo xtask verify".to_owned(),
            since: Timestamp(1_000),
        };
        let words = |p: Option<CheckPhase>, now| check_words(p.as_ref(), now);
        assert_eq!(
            words(Some(running.clone()), 1_000 + 3 * 60_000 + 5_000),
            "being checked: cargo xtask verify, 3 min"
        );
        assert_eq!(
            words(Some(running), 1_000 + 42_000),
            "being checked: cargo xtask verify, 42 s"
        );
        assert_eq!(
            words(Some(CheckPhase::Syncing), 0),
            "being checked: syncing with main"
        );
        assert_eq!(
            words(Some(CheckPhase::Queued { place: 1 }), 0),
            "being checked: queued, next in line"
        );
        assert_eq!(
            words(Some(CheckPhase::Queued { place: 3 }), 0),
            "being checked: queued, #3 in line"
        );
        assert_eq!(words(None, 0), "being checked: starting");
    }
}
