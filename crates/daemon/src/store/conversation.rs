//! A project's conversation with its assistant: the messages the store keeps itself, and a card's
//! discussion read from the card's thread, as one list ordered by time. A discussion message is
//! never copied here: it lives once, in `thread_messages`.

use rusqlite::params;
use toknila_core::ProjectId;
use toknila_core::conversation::{About, Message};
use toknila_core::{TicketKey, Timestamp};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, from_json, json};

/// The conversation as one table: the stored messages, and the plain messages of the project's
/// card discussions (the card's key is the context, the thread's number goes along).
const ALL: &str = "WITH conv(id, author, text, about_kind, about_ref, attachments_json, thread, created_at) AS (\
    SELECT id, author, text, about_kind, about_ref, attachments_json, NULL, created_at \
      FROM conversation_messages WHERE project_id = ?1 \
    UNION ALL \
    SELECT tm.id, CASE tm.author WHEN 'human' THEN 'person' WHEN 'agent' THEN 'assistant' ELSE 'toknila' END, \
           tm.text, 'card', t.key, '[]', th.number, tm.created_at \
      FROM thread_messages tm \
      JOIN threads th ON th.id = tm.thread_id AND th.kind = 'discussion' \
      JOIN tickets t ON t.id = th.ticket_id \
      JOIN chains c ON c.id = t.chain_id \
     WHERE c.project_id = ?1 AND tm.kind = 'message') ";

type Row = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    Option<u32>,
    i64,
);

fn message(r: Row) -> Result<Message, StoreError> {
    let about = match (r.3.as_deref(), r.4) {
        (Some("card"), Some(key)) => Some(About::Card {
            key: key
                .parse::<TicketKey>()
                .map_err(|_| StoreError::Corrupt(format!("bad card key: {key:?}")))?,
        }),
        (Some("chain"), Some(name)) => Some(About::Chain { name }),
        _ => None,
    };
    Ok(Message {
        id: r.0,
        author: enum_from(&r.1)?,
        text: r.2,
        about,
        attachments: from_json(&r.5)?,
        thread: r.6,
        at: Timestamp(r.7),
    })
}

impl Repo<'_> {
    /// Stores a message of the project's conversation.
    pub fn insert_conversation_message(
        &self,
        project: ProjectId,
        m: &Message,
    ) -> Result<(), StoreError> {
        let (kind, reference) = match &m.about {
            Some(About::Card { key }) => (Some("card"), Some(key.to_string())),
            Some(About::Chain { name }) => (Some("chain"), Some(name.clone())),
            None => (None, None),
        };
        self.conn.execute(
            "INSERT INTO conversation_messages (id, project_id, author, text, about_kind, about_ref, \
             attachments_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                m.id,
                project.to_string(),
                enum_str(&m.author)?,
                m.text,
                kind,
                reference,
                json(&m.attachments)?,
                m.at.0,
            ],
        )?;
        Ok(())
    }

    /// The newest `limit` messages older than `before`, oldest first (see
    /// `Reader::conversation_page`). A `before` that names no message gives an empty page.
    pub fn conversation_page(
        &self,
        project: ProjectId,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Message>, StoreError> {
        let older = if before.is_some() {
            "WHERE (created_at, id) < (SELECT created_at, id FROM conv WHERE id = ?2) "
        } else {
            "WHERE ?2 IS NULL "
        };
        let sql = format!(
            "{ALL}SELECT id, author, text, about_kind, about_ref, attachments_json, thread, created_at \
             FROM conv {older}ORDER BY created_at DESC, id DESC LIMIT ?3"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(
            params![
                project.to_string(),
                before,
                i64::try_from(limit).unwrap_or(i64::MAX)
            ],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;
        let mut out = rows.map(|r| message(r?)).collect::<Result<Vec<_>, _>>()?;
        out.reverse();
        Ok(out)
    }
}
