//! SQLite store with a single writer.
//!
//! One tokio task owns the connection. Every command runs as one transaction that changes state
//! and appends its events to the `events` journal; either both land or neither does. Committed
//! events are then published to subscribers (the WebSocket stream).

mod backup;
mod chains;
mod codec;
mod error;
mod external;
mod notes;
mod ports;
mod projects;
mod removal;
mod schema;
mod ships;
mod threads;
mod tickets;
mod usage;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, oneshot};
use toknila_app::external::ExternalEffect;
use toknila_app::runner::{NoRunner, Runner};
use toknila_app::{Merger, Shipyard};
use toknila_core::events::DomainEvent;
use toknila_core::{Actor, AgentRole, TicketId, Timestamp};

pub use codec::Repo;
pub use error::StoreError;
pub use toknila_api_types::EventRecord;
pub use usage::{FileState, RowFilter};

/// A command's view of the database: its transaction plus the events it emits.
pub struct WriteTx<'c> {
    tx: Transaction<'c>,
    actor: Actor,
    now: Timestamp,
    events: Vec<EventRecord>,
    deferred: Vec<ExternalEffect>,
    /// The runner processes alive when the command began.
    live: Vec<(TicketId, AgentRole)>,
}

/// Where a command's ships are made; shared by every command of the store.
pub type Yard = Arc<dyn Shipyard + Send + Sync>;

/// Where a command's merges run; shared by every command of the store.
pub type Merges = Arc<dyn Merger + Send + Sync>;
/// The runner of a hand-in's check command, shared with the blocking thread that runs effects.
pub type Checks = Arc<dyn toknila_app::Checker + Send + Sync>;

/// Where a command's agents run; shared by every command of the store.
pub type Agents = Arc<dyn Runner + Send + Sync>;

impl std::fmt::Debug for WriteTx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteTx")
            .field("actor", &self.actor)
            .field("events", &self.events.len())
            .finish()
    }
}

impl<'c> WriteTx<'c> {
    /// Time of this command; every row and event it writes uses the same instant.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Who issued this command.
    pub fn actor(&self) -> Actor {
        self.actor
    }

    /// Raw connection inside the transaction, for repository code.
    pub fn conn(&self) -> &Connection {
        &self.tx
    }

    /// Typed repository over this transaction.
    pub fn repo(&self) -> Repo<'_> {
        Repo::with_live(&self.tx, self.live.clone())
    }

    /// Journals a domain event in this transaction.
    pub fn emit(&mut self, event: &DomainEvent) -> Result<(), StoreError> {
        self.append(event.entity(), event.entity_id(), event.kind(), event)
    }

    /// Appends a raw journal entry; [`Self::emit`] is the typed way in.
    pub(crate) fn append(
        &mut self,
        entity: &str,
        entity_id: impl ToString,
        kind: &str,
        payload: impl Serialize,
    ) -> Result<(), StoreError> {
        let payload = serde_json::to_value(payload)?;
        let entity_id = entity_id.to_string();
        let actor = codec::enum_str(&self.actor)?;
        self.tx.execute(
            "INSERT INTO events (at, actor, entity, entity_id, kind, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.now.0, actor, entity, entity_id, kind, payload.to_string()],
        )?;
        let seq = self.tx.last_insert_rowid();
        self.events.push(EventRecord {
            seq,
            at: self.now,
            actor: self.actor,
            entity: entity.to_owned(),
            entity_id,
            kind: kind.to_owned(),
            payload,
        });
        Ok(())
    }
}

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Handle to the store task. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Store {
    jobs: mpsc::Sender<Job>,
    events: broadcast::Sender<EventRecord>,
    path: PathBuf,
    yard: Option<Yard>,
    merger: Merges,
    checker: Checks,
    runner: Agents,
}

impl Store {
    /// Opens (creating if needed) the database at `path`, runs migrations, starts the writer.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let backups = path.parent().unwrap_or(Path::new("")).join("backups");
        backup::check(&conn, path, &backups)?;
        migrate(&mut conn, &backups)?;
        Ok(Self::spawn(conn, path.to_owned()))
    }

    fn spawn(mut conn: Connection, path: PathBuf) -> Store {
        let (jobs, mut rx) = mpsc::channel::<Job>(256);
        let (events, _) = broadcast::channel(1024);
        // A dedicated thread: SQLite calls block, and this keeps the runtime's workers free.
        std::thread::Builder::new()
            .name("toknila-store".to_owned())
            .spawn(move || {
                while let Some(job) = rx.blocking_recv() {
                    job(&mut conn);
                }
            })
            .map(|_| ())
            .unwrap_or_else(|e| panic!("cannot spawn store thread: {e}"));
        Store {
            jobs,
            events,
            path,
            yard: None,
            merger: Arc::new(external::NoMerger),
            checker: Arc::new(crate::checker::ShellChecker::default()),
            runner: Arc::new(NoRunner),
        }
    }

    /// Gives every command a shipyard; without one tickets run without ships.
    pub fn with_shipyard(mut self, yard: Yard) -> Store {
        self.yard = Some(yard);
        self
    }

    /// Lets accepted cards merge into their base branch; without one a merge is a typed failure.
    pub fn with_merger(mut self, merger: Merges) -> Store {
        self.merger = merger;
        self
    }

    /// Chooses how a hand-in's check command runs; the default runs it through the shell.
    pub fn with_checker(mut self, checker: Checks) -> Store {
        self.checker = checker;
        self
    }

    /// Lets commands start and stop agents; without one a non-simulated ticket cannot start.
    pub fn with_runner(mut self, runner: Agents) -> Store {
        self.runner = runner;
        self
    }

    /// Path of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Live committed events. Pair with [`Store::events_since`] to catch up first.
    pub fn subscribe(&self) -> broadcast::Receiver<EventRecord> {
        self.events.subscribe()
    }

    /// Runs `f` as one transaction. On `Ok` the state and the emitted events commit together and
    /// the events are published; on `Err` or panic nothing is written. Work the command queued
    /// for the host runs after the commit (see [`Store::after_commit`]).
    pub async fn write<T, E, F>(
        &self,
        actor: Actor,
        now: Timestamp,
        f: F,
    ) -> Result<(T, Vec<EventRecord>), E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_>) -> Result<T, E> + Send + 'static,
    {
        let (value, emitted, queued) = self.commit_with_effects(actor, now, f).await?;
        self.after_commit(queued).await;
        Ok((value, emitted))
    }

    pub(super) async fn commit_with_effects<T, E, F>(
        &self,
        actor: Actor,
        now: Timestamp,
        f: F,
    ) -> Result<(T, Vec<EventRecord>, Vec<ExternalEffect>), E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut WriteTx<'_>) -> Result<T, E> + Send + 'static,
    {
        let events = self.events.clone();
        let live = self.runner.live_runners();
        let outcome = self
            .run(move |conn| {
                let tx = match conn.transaction() {
                    Ok(tx) => tx,
                    Err(e) => return Ok(Err(E::from(e.into()))),
                };
                let mut w = WriteTx {
                    tx,
                    actor,
                    now,
                    events: Vec::new(),
                    deferred: Vec::new(),
                    live,
                };
                let value = match catch_unwind(AssertUnwindSafe(|| f(&mut w))) {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => return Ok(Err(e)),
                    Err(p) => return Ok(Err(E::from(StoreError::Panicked(panic_message(&p))))),
                };
                let WriteTx {
                    tx,
                    events: emitted,
                    deferred,
                    ..
                } = w;
                if let Err(e) = tx.commit() {
                    return Ok(Err(E::from(e.into())));
                }
                for e in &emitted {
                    // No subscribers is fine.
                    let _ = events.send(e.clone());
                }
                Ok(Ok((value, emitted, deferred)))
            })
            .await;
        match outcome {
            Ok(inner) => inner,
            Err(e) => Err(E::from(e)),
        }
    }

    /// Runs a read-only query on the writer's connection (so it sees every committed write).
    pub async fn read<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&Repo<'_>) -> Result<T, StoreError> + Send + 'static,
    {
        let live = self.runner.live_runners();
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let out = match catch_unwind(AssertUnwindSafe(|| f(&Repo::with_live(&tx, live)))) {
                Ok(r) => r,
                Err(p) => Err(StoreError::Panicked(panic_message(&p))),
            };
            tx.rollback()?;
            out
        })
        .await
    }

    /// Journal entries with `seq > since`, oldest first.
    pub async fn events_since(
        &self,
        since: i64,
        limit: usize,
    ) -> Result<Vec<EventRecord>, StoreError> {
        self.read(move |repo| repo.events_since(since, limit)).await
    }

    async fn run<T, F>(&self, f: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, rx) = oneshot::channel();
        let job: Job = Box::new(move |conn| {
            let _ = reply.send(f(conn));
        });
        self.jobs.send(job).await.map_err(|_| StoreError::Closed)?;
        rx.await.map_err(|_| StoreError::Closed)?
    }
}

fn panic_message(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned())
}

/// Current schema version this build writes.
pub const SCHEMA_VERSION: i64 = schema::MIGRATIONS.len() as i64;

fn migrate(conn: &mut Connection, backups: &Path) -> Result<(), StoreError> {
    let found: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::TooNew {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    // A fresh database (version 0) has nothing to lose; an older one is copied first.
    if found > 0 && found < SCHEMA_VERSION {
        backup::before_migration(conn, backups, found)?;
    }
    for (i, sql) in schema::MIGRATIONS
        .iter()
        .enumerate()
        .skip(usize::try_from(found).unwrap_or(0))
    {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(
            None,
            "user_version",
            i64::try_from(i + 1).unwrap_or(i64::MAX),
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Highest journal seq, or 0 for an empty journal.
pub(crate) fn last_seq(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn
        .query_row(
            "SELECT seq FROM events ORDER BY seq DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

#[cfg(test)]
mod tests;
