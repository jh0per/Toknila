//! Row <-> domain mapping for threads on a card: threads, their messages and artifact versions.
//! Versions and messages are insert-only.

use rusqlite::params;
use toknila_core::{Thread, ThreadId, ThreadMessage, ThreadVersion, TicketId, Timestamp};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, parse};

impl Repo<'_> {
    pub fn upsert_thread(&self, t: &Thread) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO threads (id, ticket_id, number, kind, title, state, opened_by, created_at, \
             approved_version, approved_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, state = excluded.state, \
             approved_version = excluded.approved_version, approved_at = excluded.approved_at",
            params![
                t.id.to_string(),
                t.ticket_id.to_string(),
                t.number,
                enum_str(&t.kind)?,
                t.title,
                enum_str(&t.state)?,
                enum_str(&t.opened_by)?,
                t.created_at.0,
                t.approved_version,
                t.approved_at.map(|a| a.0),
            ],
        )?;
        Ok(())
    }

    pub fn threads(&self, ticket: TicketId) -> Result<Vec<Thread>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, ticket_id, number, kind, title, state, opened_by, created_at, \
             approved_version, approved_at FROM threads WHERE ticket_id = ?1 ORDER BY number",
        )?;
        let rows = st.query_map([ticket.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, Option<u32>>(8)?,
                r.get::<_, Option<i64>>(9)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(Thread {
                id: parse(&r.0, "thread id")?,
                ticket_id: parse(&r.1, "ticket id")?,
                number: r.2,
                kind: enum_from(&r.3)?,
                title: r.4,
                state: enum_from(&r.5)?,
                opened_by: enum_from(&r.6)?,
                created_at: Timestamp(r.7),
                approved_version: r.8,
                approved_at: r.9.map(Timestamp),
            })
        })
        .collect()
    }

    pub fn insert_thread_message(&self, m: &ThreadMessage) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO thread_messages (id, thread_id, author, kind, text, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                m.id.to_string(),
                m.thread_id.to_string(),
                enum_str(&m.author)?,
                enum_str(&m.kind)?,
                m.text,
                m.created_at.0,
            ],
        )?;
        Ok(())
    }

    pub fn thread_messages(&self, thread: ThreadId) -> Result<Vec<ThreadMessage>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, thread_id, author, kind, text, created_at FROM thread_messages \
             WHERE thread_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = st.query_map([thread.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(ThreadMessage {
                id: parse(&r.0, "message id")?,
                thread_id: parse(&r.1, "thread id")?,
                author: enum_from(&r.2)?,
                kind: enum_from(&r.3)?,
                text: r.4,
                created_at: Timestamp(r.5),
            })
        })
        .collect()
    }

    pub fn insert_thread_version(&self, v: &ThreadVersion) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO thread_versions (id, thread_id, number, author, format, body, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                v.id.to_string(),
                v.thread_id.to_string(),
                v.number,
                enum_str(&v.author)?,
                enum_str(&v.format)?,
                v.body,
                v.created_at.0,
            ],
        )?;
        Ok(())
    }

    pub fn thread_versions(&self, thread: ThreadId) -> Result<Vec<ThreadVersion>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, thread_id, number, author, format, body, created_at FROM thread_versions \
             WHERE thread_id = ?1 ORDER BY number",
        )?;
        let rows = st.query_map([thread.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(ThreadVersion {
                id: parse(&r.0, "version id")?,
                thread_id: parse(&r.1, "thread id")?,
                number: r.2,
                author: enum_from(&r.3)?,
                format: enum_from(&r.4)?,
                body: r.5,
                created_at: Timestamp(r.6),
            })
        })
        .collect()
    }
}
