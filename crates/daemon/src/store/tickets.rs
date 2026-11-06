//! Row <-> domain mapping for tickets.

use rusqlite::params;
use toknila_core::{ChainId, Ticket, TicketId, TicketKey, Timestamp};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, from_json, json, opt_enum, opt_enum_from, parse};

const TICKET_COLS: &str = "id, chain_id, key, title, prompt, acceptance_json, status, status_reason_json, \
     status_since, paused, profile, review_mode, branch, needs_secrets, tokens_used, wildcard_json, \
     done_out_of_order_json, queued";

impl Repo<'_> {
    /// All tickets, optionally of one chain, in key order.
    pub fn tickets(&self, chain: Option<ChainId>) -> Result<Vec<Ticket>, StoreError> {
        let mut out = match chain {
            Some(c) => self.tickets_where("chain_id = ?1", c.to_string())?,
            None => self.tickets_where("?1 = ?1", String::new())?,
        };
        out.sort_by(|a, b| {
            (a.key.project(), a.key.number()).cmp(&(b.key.project(), b.key.number()))
        });
        Ok(out)
    }

    pub fn upsert_ticket(&self, t: &Ticket) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO tickets ({TICKET_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18) \
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, prompt = excluded.prompt, \
                 acceptance_json = excluded.acceptance_json, status = excluded.status, \
                 status_reason_json = excluded.status_reason_json, status_since = excluded.status_since, \
                 paused = excluded.paused, profile = excluded.profile, review_mode = excluded.review_mode, \
                 branch = excluded.branch, needs_secrets = excluded.needs_secrets, tokens_used = excluded.tokens_used, \
                 wildcard_json = excluded.wildcard_json, \
                 done_out_of_order_json = excluded.done_out_of_order_json, queued = excluded.queued"
            ),
            params![
                t.id.to_string(),
                t.chain_id.to_string(),
                t.key.to_string(),
                t.title,
                t.prompt,
                json(&t.acceptance)?,
                enum_str(&t.status)?,
                t.status_reason.as_ref().map(json).transpose()?,
                t.status_since.0,
                t.paused,
                t.profile,
                opt_enum(&t.review_mode)?,
                t.branch,
                t.needs_secrets,
                i64::try_from(t.tokens_used).unwrap_or(i64::MAX),
                t.wildcard.as_ref().map(json).transpose()?,
                t.done_out_of_order.as_ref().map(json).transpose()?,
                t.queued,
            ],
        )?;
        Ok(())
    }

    pub(super) fn tickets_where(
        &self,
        clause: &str,
        arg: String,
    ) -> Result<Vec<Ticket>, StoreError> {
        type Raw = (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            i64,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
            i64,
            Option<String>,
            Option<String>,
            bool,
        );
        let mut st = self
            .conn
            .prepare(&format!("SELECT {TICKET_COLS} FROM tickets WHERE {clause}"))?;
        let rows = st.query_map([arg], |r| -> rusqlite::Result<Raw> {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
                r.get(12)?,
                r.get(13)?,
                r.get(14)?,
                r.get(15)?,
                r.get(16)?,
                r.get(17)?,
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(Ticket {
                id: parse(&r.0, "ticket id")?,
                chain_id: parse(&r.1, "chain id")?,
                key: parse(&r.2, "ticket key")?,
                title: r.3,
                prompt: r.4,
                acceptance: from_json(&r.5)?,
                status: enum_from(&r.6)?,
                status_reason: r.7.as_deref().map(from_json).transpose()?,
                status_since: Timestamp(r.8),
                paused: r.9,
                queued: r.17,
                profile: r.10,
                review_mode: opt_enum_from(r.11)?,
                branch: r.12,
                needs_secrets: r.13,
                tokens_used: u64::try_from(r.14).unwrap_or(0),
                wildcard: r.15.as_deref().map(from_json).transpose()?,
                done_out_of_order: r.16.as_deref().map(from_json).transpose()?,
            })
        })
        .collect()
    }

    pub fn ticket(&self, id: TicketId) -> Result<Option<Ticket>, StoreError> {
        Ok(self.tickets_where("id = ?1", id.to_string())?.pop())
    }

    pub fn ticket_by_key(&self, key: &TicketKey) -> Result<Option<Ticket>, StoreError> {
        Ok(self.tickets_where("key = ?1", key.to_string())?.pop())
    }
}
