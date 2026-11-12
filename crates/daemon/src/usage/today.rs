//! Today's spend for the window's top bar: the open project's, the machine's and the plan
//! windows, by the person's local day. Cached for a minute and dropped when a run ends.

use std::time::{Duration, Instant};

use toknila_api_types::{QuotaReport, TodayUsage};
use toknila_core::Timestamp;
use toknila_core::usage::quota::{self, QuotaSnapshot};
use toknila_core::usage::{today_figures, today_per_card};

use super::{QUOTA, Usage, machine_scan};
use crate::server::now;
use crate::store::{RowFilter, Store, StoreError};

const DAY_MS: i64 = 86_400_000;
/// How long an answer is kept.
const KEEP: Duration = Duration::from_secs(60);
/// A quota report older than this is no longer shown on the line.
const FRESH_MS: i64 = 3_600_000;

/// One kept answer: for which project and clock, when, and after how many runs had ended.
#[derive(Debug)]
pub(super) struct Kept {
    key: (String, i32),
    at: Instant,
    ended: i64,
    answer: TodayUsage,
}

impl Usage {
    /// Today's figures of `project_key` and of the machine, for a clock `offset_minutes` east of
    /// UTC, and the plan windows reported within the hour.
    pub async fn today(
        &self,
        store: &Store,
        project_key: &str,
        offset_minutes: i32,
    ) -> Result<TodayUsage, StoreError> {
        let key = (project_key.to_owned(), offset_minutes);
        let ended = store.read(|repo| repo.runs_ended()).await?;
        if let Some(kept) = self.kept(&key, ended) {
            return Ok(kept);
        }
        self.refresh(store, super::Scope::All).await?;
        let (prices, _) = self.prices();
        let at = now();
        let since = Timestamp(at.0 - 2 * DAY_MS);
        let (rows, snapshot) = store
            .read(move |repo| {
                let rows = repo.usage_rows(RowFilter {
                    since: Some(since),
                    attributed_only: !machine_scan(repo)?,
                    ..RowFilter::default()
                })?;
                Ok((rows, repo.app_setting(QUOTA)?))
            })
            .await?;
        let mine: Vec<_> = rows
            .iter()
            .filter(|r| r.project.as_deref() == Some(project_key))
            .cloned()
            .collect();
        let answer = TodayUsage {
            project: today_figures(&mine, &prices, at, offset_minutes),
            machine: today_figures(&rows, &prices, at, offset_minutes),
            quota: fresh_quota(snapshot, at),
            cards: today_per_card(&mine, &prices, at, offset_minutes),
        };
        self.keep(Kept {
            key,
            at: Instant::now(),
            ended,
            answer: answer.clone(),
        });
        Ok(answer)
    }

    fn kept(&self, key: &(String, i32), ended: i64) -> Option<TodayUsage> {
        let cache = self.today.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .iter()
            .find(|k| &k.key == key && k.ended == ended && k.at.elapsed() < KEEP)
            .map(|k| k.answer.clone())
    }

    /// Drops every kept answer: a new quota report or a changed setting makes them stale.
    pub(super) fn forget_today(&self) {
        self.today.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn keep(&self, kept: Kept) {
        let mut cache = self.today.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|k| k.key != kept.key && k.at.elapsed() < KEEP);
        cache.push(kept);
    }
}

/// The windows a status line reported within the hour and that have not reset since.
fn fresh_quota(snapshot: Option<String>, at: Timestamp) -> Option<QuotaReport> {
    let snapshot: QuotaSnapshot = serde_json::from_str(&snapshot?).ok()?;
    if at.0 - snapshot.seen_at.0 > FRESH_MS {
        return None;
    }
    let windows = quota::view(Some(&snapshot), at).windows;
    (!windows.is_empty()).then_some(QuotaReport { windows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::usage::quota::QuotaWindow;

    fn snapshot(seen_at: i64) -> Option<String> {
        let windows = vec![QuotaWindow {
            name: "five_hour".to_owned(),
            used_percentage: 48.0,
            resets_at: 4_102_444_800,
        }];
        let snap = QuotaSnapshot {
            seen_at: Timestamp(seen_at),
            windows,
        };
        Some(serde_json::to_string(&snap).expect("json"))
    }

    #[test]
    fn a_report_older_than_an_hour_is_not_shown() {
        let at = Timestamp(10_000_000);
        let fresh = fresh_quota(snapshot(at.0 - 59 * 60_000), at).expect("fresh");
        assert_eq!(fresh.windows[0].used_percentage, 48.0);
        assert!(fresh_quota(snapshot(at.0 - 61 * 60_000), at).is_none());
        assert!(fresh_quota(None, at).is_none());
    }
}
