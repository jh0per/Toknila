//! Row <-> domain mapping for token usage: sessions (which run an agent session is), the
//! transcript files read so far, one row per model call, and machine-wide settings.

use rusqlite::{OptionalExtension, params};
use toknila_core::usage::{Record, Row, TokenCounts};
use toknila_core::{AgentRole, ChainId, RunId, TicketId, Timestamp};

use super::StoreError;
use super::codec::{Repo, enum_str, parse};

/// A transcript file as the store knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub path: String,
    /// Bytes read so far.
    pub offset: u64,
    pub workspace: Option<String>,
}

/// Which model calls to load.
#[derive(Debug, Clone, Copy, Default)]
pub struct RowFilter {
    /// Only calls at or after this time.
    pub since: Option<Timestamp>,
    /// Only calls made by a Toknila run.
    pub attributed_only: bool,
    pub ticket: Option<TicketId>,
    pub chain: Option<ChainId>,
}

fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

fn db_int(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl Repo<'_> {
    /// Ties a Claude Code session to a ticket's run, and adopts the calls of its files that were
    /// read before the session was known.
    pub fn usage_session_set(
        &self,
        session: &str,
        ticket: TicketId,
        run: Option<RunId>,
        role: AgentRole,
        transcript: &str,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO usage_sessions (session_id, ticket_id, run_id, role, transcript_path, at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(session_id) DO UPDATE SET ticket_id = excluded.ticket_id, \
             run_id = excluded.run_id, role = excluded.role, transcript_path = excluded.transcript_path, at = excluded.at",
            params![session, ticket.to_string(), run.map(|r| r.to_string()), enum_str(&role)?, transcript, at.0],
        )?;
        self.conn.execute(
            "UPDATE usage_records SET ticket_id = ?1 WHERE ticket_id IS NULL AND file IN \
             (SELECT path FROM usage_files WHERE session_id = ?2)",
            params![ticket.to_string(), session],
        )?;
        Ok(())
    }

    /// The ticket a session belongs to, when a hook said so.
    pub fn usage_session_ticket(&self, session: &str) -> Result<Option<TicketId>, StoreError> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT ticket_id FROM usage_sessions WHERE session_id = ?1",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        id.map(|id| parse(&id, "ticket id")).transpose()
    }

    /// The transcript path each attributed session reported.
    pub fn usage_session_paths(&self) -> Result<Vec<String>, StoreError> {
        let mut st = self
            .conn
            .prepare("SELECT transcript_path FROM usage_sessions ORDER BY at")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every transcript file read so far.
    pub fn usage_files(&self) -> Result<Vec<FileState>, StoreError> {
        let mut st = self
            .conn
            .prepare("SELECT path, read_offset, workspace FROM usage_files")?;
        let rows = st.query_map([], |r| {
            Ok(FileState {
                path: r.get(0)?,
                offset: count(r.get(1)?),
                workspace: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Saves what one read of a transcript found: the file's new offset and workspace, and its
    /// calls. A call seen again replaces the earlier one (a later line is the more complete);
    /// it keeps the ticket it was first attributed to.
    pub fn usage_ingest(
        &self,
        file: &FileState,
        kind: &str,
        session: Option<&str>,
        ticket: Option<TicketId>,
        records: &[Record],
    ) -> Result<(), StoreError> {
        let offset = db_int(file.offset);
        self.conn.execute(
            "INSERT INTO usage_files (path, kind, session_id, read_offset, workspace) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(path) DO UPDATE SET read_offset = excluded.read_offset, \
             workspace = COALESCE(usage_files.workspace, excluded.workspace)",
            params![file.path, kind, session, offset, file.workspace],
        )?;
        let mut put = self.conn.prepare_cached(
            "INSERT INTO usage_records (key, file, ticket_id, at, model, input, cache_write, cache_read, output) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT(key) DO UPDATE SET at = excluded.at, \
             model = excluded.model, input = excluded.input, cache_write = excluded.cache_write, \
             cache_read = excluded.cache_read, output = excluded.output",
        )?;
        for r in records {
            put.execute(params![
                r.key,
                file.path,
                ticket.map(|t| t.to_string()),
                r.at.0,
                r.model,
                db_int(r.tokens.input),
                db_int(r.tokens.cache_write),
                db_int(r.tokens.cache_read),
                db_int(r.tokens.output)
            ])?;
        }
        Ok(())
    }

    /// The model calls matching `filter`, with what each belongs to.
    pub fn usage_rows(&self, filter: RowFilter) -> Result<Vec<Row>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT r.at, r.model, r.input, r.cache_write, r.cache_read, r.output, \
                    t.key, c.name, p.key, COALESCE(p.repo_path, f.workspace, '') \
             FROM usage_records r \
             JOIN usage_files f ON f.path = r.file \
             LEFT JOIN tickets t ON t.id = r.ticket_id \
             LEFT JOIN chains c ON c.id = t.chain_id \
             LEFT JOIN projects p ON p.id = c.project_id \
             WHERE r.at >= ?1 AND (?2 = 0 OR r.ticket_id IS NOT NULL) \
               AND (?3 IS NULL OR r.ticket_id = ?3) AND (?4 IS NULL OR t.chain_id = ?4)",
        )?;
        let rows = st.query_map(
            params![
                filter.since.map_or(0, |t| t.0),
                filter.attributed_only,
                filter.ticket.map(|t| t.to_string()),
                filter.chain.map(|c| c.to_string())
            ],
            |r| {
                Ok(Row {
                    at: Timestamp(r.get(0)?),
                    model: r.get(1)?,
                    tokens: TokenCounts {
                        input: count(r.get(2)?),
                        cache_write: count(r.get(3)?),
                        cache_read: count(r.get(4)?),
                        output: count(r.get(5)?),
                    },
                    card: r.get(6)?,
                    chain: r.get(7)?,
                    project: r.get(8)?,
                    workspace: r.get(9)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A machine-wide setting stored as JSON text.
    pub fn app_setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Saves a machine-wide setting.
    pub fn app_setting_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
