//! How long a card's agents ran: the sum of its finished runs, and the start of the run that is
//! still going. Plain data in and out; the daemon reads the journal's runner events.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// A runner process of a card starting or ending, as the journal says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMark {
    /// Which run (a worker's and a reviewer's runs of one card are different runs).
    pub run: String,
    pub at: Timestamp,
    /// `true` for the end of the process, `false` for its start.
    pub ended: bool,
}

/// The time a card's runs took.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunTime {
    /// Milliseconds the finished runs lasted, together.
    pub done_ms: u64,
    /// When the newest run that has not ended started; `null` while nothing runs.
    pub since: Option<Timestamp>,
}

/// The run time of one card from its runner events, oldest first. A run that started and never
/// ended (the daemon stopped) counts as running only when `live` says a process is alive; with no
/// live process its open start is ignored.
pub fn run_time(marks: &[RunMark], live: bool) -> RunTime {
    let mut out = RunTime::default();
    let mut open: Vec<(&str, Timestamp)> = Vec::new();
    for m in marks {
        if m.ended {
            if let Some(i) = open.iter().position(|(r, _)| *r == m.run) {
                let (_, start) = open.remove(i);
                out.done_ms += u64::try_from((m.at.0 - start.0).max(0)).unwrap_or(0);
            }
        } else {
            open.push((&m.run, m.at));
        }
    }
    if live {
        out.since = open.iter().map(|(_, at)| *at).max();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(run: &str, at: i64, ended: bool) -> RunMark {
        RunMark {
            run: run.to_owned(),
            at: Timestamp(at),
            ended,
        }
    }

    #[test]
    fn finished_runs_add_up_and_a_live_one_is_the_current() {
        let marks = [
            mark("a", 1_000, false),
            mark("a", 4_000, true),
            mark("b", 10_000, false),
            mark("b", 12_500, true),
            mark("c", 20_000, false),
        ];
        let idle = run_time(&marks, false);
        assert_eq!((idle.done_ms, idle.since), (5_500, None));
        let live = run_time(&marks, true);
        assert_eq!((live.done_ms, live.since), (5_500, Some(Timestamp(20_000))));
    }
}
