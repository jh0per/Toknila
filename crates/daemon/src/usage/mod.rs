//! Token usage: which agent sessions are Toknila's runs, reading their transcripts, and the
//! numbers served to the page. The parsing and the sums are core's (`toknila_core::usage`); this
//! is the adapter that reads the files and stores what they hold.
//!
//! Transcripts are read from a stored offset, so each pass reads only what was appended. An agent
//! kind is a [`Source`]: its transcript format and the folders it writes to. Claude Code only for
//! now; Codex (`~/.codex/sessions`) would be one more source.

mod refresh;
mod repo;
mod scan;
mod watch;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use toknila_api_types::{PriceInfo, QuotaReport, UsageView};
use toknila_core::usage::quota::{self, QuotaSnapshot, QuotaView};
use toknila_core::usage::{ClaudeCode, PriceTable, TranscriptFormat, summarize};
use toknila_core::{Actor, AgentRole, RunId, TicketId, Timestamp};

pub use refresh::Scope;
pub use watch::watch;

use crate::server::now;
use crate::store::{Repo, RowFilter, Store, StoreError};

const MACHINE_SCAN: &str = "usage.machine_scan";
const QUOTA: &str = "usage.quota";
const DAY_MS: i64 = 86_400_000;

/// One agent kind's transcripts.
#[derive(Clone)]
pub struct Source {
    pub format: Arc<dyn TranscriptFormat>,
    /// Where the agent writes them.
    pub roots: Vec<PathBuf>,
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("kind", &self.format.kind())
            .field("roots", &self.roots)
            .finish()
    }
}

/// Token usage: the sources, the price overrides file and the guard of the reader.
pub struct Usage {
    prices_file: PathBuf,
    sources: Vec<Source>,
    gate: tokio::sync::Mutex<()>,
    /// Sessions already tied to their run, so a hook does not write the same row again.
    known: Mutex<HashSet<String>>,
    /// The repository of each workspace path, asked of git once.
    repos: Arc<repo::RepoCache>,
}

impl std::fmt::Debug for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Usage")
            .field("sources", &self.sources.len())
            .finish()
    }
}

/// Claude Code's folder of transcripts: `$CLAUDE_CONFIG_DIR/projects`, else `~/.claude/projects`.
fn claude_projects() -> Option<PathBuf> {
    let config = match std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => dirs::home_dir()?.join(".claude"),
    };
    Some(config.join("projects"))
}

/// Whether every session on the machine is counted (on unless a person switched it off).
pub(crate) fn machine_scan(repo: &Repo<'_>) -> Result<bool, StoreError> {
    Ok(repo.app_setting(MACHINE_SCAN)?.as_deref() != Some("false"))
}

impl Usage {
    /// The production setup: Claude Code's transcripts, and `usage-prices.toml` in `home`.
    pub fn new(home: &Path) -> Usage {
        let roots = claude_projects().into_iter().collect();
        Usage::with_sources(
            home,
            vec![Source {
                format: Arc::new(ClaudeCode),
                roots,
            }],
        )
    }

    /// A setup that reads no transcripts (tests that do not care about usage).
    pub fn disabled() -> Usage {
        Usage::with_sources(Path::new(""), vec![])
    }

    /// A setup over the given sources (tests point them at a temporary folder).
    pub fn with_sources(home: &Path, sources: Vec<Source>) -> Usage {
        Usage {
            prices_file: home.join("usage-prices.toml"),
            sources,
            gate: tokio::sync::Mutex::new(()),
            known: Mutex::new(HashSet::new()),
            repos: Arc::default(),
        }
    }

    /// The shipped prices with the person's overrides on top, read now so an edit applies at
    /// once. A file that cannot be read or parsed is reported, and the shipped prices stay.
    pub fn prices(&self) -> (PriceTable, PriceInfo) {
        let shipped = PriceTable::defaults();
        let empty = PriceTable {
            source: String::new(),
            retrieved: String::new(),
            models: vec![],
        };
        let (over, problem) = match std::fs::read_to_string(&self.prices_file) {
            Ok(text) => match PriceTable::parse(&text) {
                Ok(t) => (t, None),
                Err(e) => (empty, Some(e.to_string())),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (empty, None),
            Err(e) => (empty, Some(e.to_string())),
        };
        let mut info = PriceInfo::new(&shipped, &over);
        info.problem = problem;
        (shipped.with_overrides(over), info)
    }

    /// Ties the Claude Code session a hook reported to the run that made it, and reads its
    /// transcript. Only a transcript inside the agent's own folder is read (the path comes from
    /// the agent's process). Called for every hook; the transcript is read again on the hooks
    /// that end a reply.
    pub async fn attribute(
        &self,
        store: &Store,
        ticket: TicketId,
        run: Option<(RunId, AgentRole)>,
        report: (&str, &str),
        signal: &str,
    ) -> Result<(), StoreError> {
        let (session, transcript) = report;
        let path = PathBuf::from(transcript);
        if !self.sources.iter().any(|s| s.accepts(&path)) {
            return Ok(());
        }
        let first = self
            .known
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.to_owned());
        if first {
            let (session, transcript) = (session.to_owned(), transcript.to_owned());
            let (run_id, role) = run.map_or((None, AgentRole::Worker), |(r, role)| (Some(r), role));
            let saved = store
                .write(Actor::System, now(), move |tx| -> Result<(), StoreError> {
                    tx.repo().usage_session_set(
                        &session,
                        ticket,
                        run_id,
                        role,
                        &transcript,
                        tx.now(),
                    )
                })
                .await;
            if let Err(e) = saved {
                self.forget(report.0);
                return Err(e);
            }
        }
        if first || matches!(signal, "hook:stop" | "hook:subagent_stop") {
            self.refresh(store, Scope::Session(path)).await?;
        }
        Ok(())
    }

    fn forget(&self, session: &str) {
        self.known
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(session);
    }

    /// The usage page: totals, series and breakdowns of the last `days` days, after reading what
    /// the transcripts gained.
    pub async fn view(&self, store: &Store, days: u32) -> Result<UsageView, StoreError> {
        self.refresh(store, Scope::All).await?;
        let (prices, info) = self.prices();
        let at = now();
        let since = Timestamp(at.0 - i64::from(days) * DAY_MS);
        let (machine, rows, snapshot) = store
            .read(move |repo| {
                let machine = machine_scan(repo)?;
                let rows = repo.usage_rows(RowFilter {
                    since: Some(since),
                    attributed_only: !machine,
                    ..RowFilter::default()
                })?;
                Ok((machine, rows, repo.app_setting(QUOTA)?))
            })
            .await?;
        let snapshot: Option<QuotaSnapshot> = snapshot.and_then(|s| serde_json::from_str(&s).ok());
        let mut summary = summarize(&rows, &prices, at, days);
        let repos = self.repos.clone();
        summary = tokio::task::spawn_blocking(move || {
            repos.annotate(&mut summary.by_workspace);
            summary
        })
        .await
        .map_err(|e| StoreError::Panicked(e.to_string()))?;
        Ok(UsageView {
            summary,
            machine_scan: machine,
            prices: info,
            quota: quota::view(snapshot.as_ref(), at),
        })
    }

    /// Switches the machine-wide view on or off.
    pub async fn set_machine_scan(&self, store: &Store, on: bool) -> Result<(), StoreError> {
        store
            .write(Actor::Human, now(), move |tx| -> Result<(), StoreError> {
                tx.repo()
                    .app_setting_set(MACHINE_SCAN, if on { "true" } else { "false" })
            })
            .await?;
        Ok(())
    }

    /// Keeps the plan limits an agent's status line reported.
    pub async fn report_quota(&self, store: &Store, report: QuotaReport) -> Result<(), StoreError> {
        let at = now();
        let snapshot = QuotaSnapshot {
            seen_at: at,
            windows: report.windows,
        };
        let json = serde_json::to_string(&snapshot)?;
        store
            .write(Actor::Agent, at, move |tx| -> Result<(), StoreError> {
                tx.repo().app_setting_set(QUOTA, &json)
            })
            .await?;
        Ok(())
    }

    /// The plan quota as the page shows it.
    pub async fn quota(&self, store: &Store) -> Result<QuotaView, StoreError> {
        let snapshot = store.read(|repo| repo.app_setting(QUOTA)).await?;
        let snapshot: Option<QuotaSnapshot> = snapshot.and_then(|s| serde_json::from_str(&s).ok());
        Ok(quota::view(snapshot.as_ref(), now()))
    }
}
