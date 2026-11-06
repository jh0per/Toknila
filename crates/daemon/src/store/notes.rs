//! Row <-> domain mapping for what is written on a ticket: subtasks and comments.

use rusqlite::params;
use toknila_core::{Comment, CommentId, Subtask, TicketId, Timestamp};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, from_json, json, opt_enum, opt_enum_from, parse};

impl Repo<'_> {
    // ---- subtasks ----

    pub fn upsert_subtask(&self, s: &Subtask) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO subtasks (id, ticket_id, position, text, done, done_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(id) DO UPDATE SET ticket_id = excluded.ticket_id, position = excluded.position, text = excluded.text, \
             done = excluded.done, done_by = excluded.done_by",
            params![s.id.to_string(), s.ticket_id.to_string(), s.position, s.text, s.done, opt_enum(&s.done_by)?],
        )?;
        Ok(())
    }

    pub fn subtasks(&self, ticket: TicketId) -> Result<Vec<Subtask>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, ticket_id, position, text, done, done_by FROM subtasks WHERE ticket_id = ?1 ORDER BY position",
        )?;
        let rows = st.query_map([ticket.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(Subtask {
                id: parse(&r.0, "subtask id")?,
                ticket_id: parse(&r.1, "ticket id")?,
                position: r.2,
                text: r.3,
                done: r.4,
                done_by: opt_enum_from(r.5)?,
            })
        })
        .collect()
    }

    // ---- comments ----

    pub fn upsert_comment(&self, c: &Comment) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO comments (id, ticket_id, author_kind, kind, body_json, created_at, delivered_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET delivered_at = excluded.delivered_at",
            params![
                c.id.to_string(),
                c.ticket_id.to_string(),
                enum_str(&c.author)?,
                enum_str(&c.kind)?,
                json(&c.body)?,
                c.created_at.0,
                c.delivered_at.map(|t| t.0),
            ],
        )?;
        Ok(())
    }

    pub fn comments(&self, ticket: TicketId) -> Result<Vec<Comment>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, ticket_id, author_kind, kind, body_json, created_at, delivered_at FROM comments \
             WHERE ticket_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = st.query_map([ticket.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<i64>>(6)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(Comment {
                id: parse::<CommentId>(&r.0, "comment id")?,
                ticket_id: parse(&r.1, "ticket id")?,
                author: enum_from(&r.2)?,
                kind: enum_from(&r.3)?,
                body: from_json(&r.4)?,
                created_at: Timestamp(r.5),
                delivered_at: r.6.map(Timestamp),
            })
        })
        .collect()
    }
}
