//! Bringing the store up to date with the transcripts: find the files, read what was appended
//! since the stored offset (off the async workers), and save the model calls in one command.

use std::collections::HashMap;
use std::path::PathBuf;

use toknila_core::Actor;
use toknila_core::usage::{accepted_transcript, session_of};

use super::scan::{self, FileRead};
use super::{Source, Usage, machine_scan};
use crate::server::now;
use crate::store::{FileState, Store, StoreError};

/// How many files are saved in one command, so the writer is never held for long.
const BATCH: usize = 40;

/// How many bytes of transcripts one pass over the machine reads before it stops.
const BUDGET: u64 = 8 * 1024 * 1024;

/// The pause after each file read, so a first start over years of transcripts stays light.
const PAUSE: std::time::Duration = std::time::Duration::from_millis(5);

/// What to bring up to date.
#[derive(Debug, Clone)]
pub enum Scope {
    /// Every session of a Toknila run, and every session on this machine when that is on.
    All,
    /// One session: its transcript and the files under the folder named like it.
    Session(PathBuf),
}

impl Source {
    fn roots(&self) -> Vec<String> {
        self.roots.iter().map(|r| r.display().to_string()).collect()
    }

    pub(super) fn accepts(&self, path: &std::path::Path) -> bool {
        accepted_transcript(&path.display().to_string(), &self.roots())
    }
}

impl Usage {
    /// Reads what is new in the transcripts in `scope` and saves it. One run at a time. `true`
    /// when the budget of a pass left more to read.
    pub async fn refresh(&self, store: &Store, scope: Scope) -> Result<bool, StoreError> {
        let _one_at_a_time = self.gate.lock().await;
        let (machine, attributed, files) = store
            .read(|repo| {
                Ok((
                    machine_scan(repo)?,
                    repo.usage_session_paths()?,
                    repo.usage_files()?,
                ))
            })
            .await?;
        let offsets: HashMap<String, u64> = files.into_iter().map(|f| (f.path, f.offset)).collect();
        let sources = self.sources.clone();
        let bounded = matches!(scope, Scope::All);
        let (reads, more) = tokio::task::spawn_blocking(move || {
            let mut reads: Vec<(usize, FileRead)> = Vec::new();
            let mut spent = 0u64;
            let mut more = false;
            for (i, path) in candidates(&sources, &scope, machine, &attributed) {
                // A pass over the whole machine takes a small budget; the rest follows next tick.
                if bounded && spent >= BUDGET {
                    more = true;
                    break;
                }
                let offset = offsets
                    .get(&path.display().to_string())
                    .copied()
                    .unwrap_or(0);
                if let Some(read) = scan::read_new(sources[i].format.as_ref(), &path, offset) {
                    spent += read.chunk.consumed as u64;
                    more |= read.more;
                    reads.push((i, read));
                    // Low priority: leave the cores to the agents between files.
                    if bounded {
                        std::thread::sleep(PAUSE);
                    }
                }
            }
            (reads, more)
        })
        .await
        .map_err(|e| StoreError::Panicked(e.to_string()))?;
        for batch in reads.chunks(BATCH) {
            let batch = batch.to_vec();
            let sources = self.sources.clone();
            store
                .write(Actor::System, now(), move |tx| -> Result<(), StoreError> {
                    let repo = tx.repo();
                    for (i, read) in &batch {
                        save(&repo, &sources[*i], read)?;
                    }
                    Ok(())
                })
                .await?;
        }
        Ok(more)
    }
}

/// The files to look at, with the index of the source that reads each.
fn candidates(
    sources: &[Source],
    scope: &Scope,
    machine: bool,
    attributed: &[String],
) -> Vec<(usize, PathBuf)> {
    let mut found: Vec<(usize, PathBuf)> = Vec::new();
    match scope {
        Scope::All if machine => {
            for (i, s) in sources.iter().enumerate() {
                found.extend(
                    s.roots
                        .iter()
                        .flat_map(|r| scan::discover(r))
                        .map(|p| (i, p)),
                );
            }
        }
        Scope::All => {
            for path in attributed {
                found.extend(session_files(sources, &PathBuf::from(path)));
            }
        }
        Scope::Session(path) => found.extend(session_files(sources, path)),
    }
    found.sort();
    found.dedup();
    found
}

/// A session's transcript and the files below the folder named like it, if its path is one a
/// source accepts.
fn session_files(sources: &[Source], transcript: &std::path::Path) -> Vec<(usize, PathBuf)> {
    let Some(i) = sources.iter().position(|s| s.accepts(transcript)) else {
        return vec![];
    };
    let mut files = vec![(i, transcript.to_owned())];
    if let Some(folder) = transcript.with_extension("").to_str().map(PathBuf::from) {
        files.extend(scan::discover(&folder).into_iter().map(|p| (i, p)));
    }
    files
}

/// Saves one file's new calls, attributed to the ticket of its session when a hook named it.
fn save(repo: &crate::store::Repo<'_>, source: &Source, read: &FileRead) -> Result<(), StoreError> {
    let path = read.path.display().to_string();
    let session = session_of(&path, &source.roots());
    let ticket = match &session {
        Some(s) => repo.usage_session_ticket(s)?,
        None => None,
    };
    let state = FileState {
        path,
        offset: read.next_offset(),
        workspace: read.chunk.cwd.clone(),
    };
    repo.usage_ingest(
        &state,
        source.format.kind(),
        session.as_deref(),
        ticket,
        &read.chunk.records,
    )
}
