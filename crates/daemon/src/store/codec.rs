//! Row <-> domain mapping. Enums are stored as their serde names; nested values as JSON.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use toknila_core::graph::ChainView;
use toknila_core::wire::wire_name;
use toknila_core::{Chain, ChainId, Edge, NodeRef, ProjectId, Release, ReleaseId, Timestamp};

use super::{EventRecord, StoreError};

/// Serde name of a unit enum value (`TicketStatus::Running` -> `"running"`).
pub(crate) fn enum_str<T: Serialize + std::fmt::Debug>(value: &T) -> Result<String, StoreError> {
    wire_name(value)
        .ok_or_else(|| StoreError::Corrupt(format!("expected a unit enum, got {value:?}")))
}

pub(super) fn enum_from<T: DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        s.to_owned(),
    ))?)
}

pub(super) fn json<T: Serialize>(v: &T) -> Result<String, StoreError> {
    Ok(serde_json::to_string(v)?)
}

pub(super) fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_str(s)?)
}

pub(super) fn parse<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, StoreError> {
    s.parse()
        .map_err(|_| StoreError::Corrupt(format!("bad {what}: {s:?}")))
}

pub(super) fn opt_enum<T: Serialize + std::fmt::Debug>(
    v: &Option<T>,
) -> Result<Option<String>, StoreError> {
    v.as_ref().map(enum_str).transpose()
}

pub(super) fn opt_enum_from<T: DeserializeOwned>(
    v: Option<String>,
) -> Result<Option<T>, StoreError> {
    v.as_deref().map(enum_from).transpose()
}

fn node_parts(n: &NodeRef) -> (&'static str, String) {
    match n {
        NodeRef::Ticket(id) => ("ticket", id.to_string()),
        NodeRef::Release(id) => ("release", id.to_string()),
    }
}

fn node_from(kind: &str, id: &str) -> Result<NodeRef, StoreError> {
    match kind {
        "ticket" => Ok(NodeRef::Ticket(parse(id, "ticket id")?)),
        "release" => Ok(NodeRef::Release(parse(id, "release id")?)),
        other => Err(StoreError::Corrupt(format!("bad node kind {other:?}"))),
    }
}

/// Typed queries over one connection or transaction.
#[derive(Debug)]
pub struct Repo<'c> {
    pub(crate) conn: &'c Connection,
}

const CHAIN_COLS: &str = "id, project_id, name, goal, autostart, review_mode";
const RELEASE_COLS: &str =
    "id, chain_id, version, channel, start_mode, state, branch, cut_at, released_at";

impl<'c> Repo<'c> {
    pub(crate) fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Reserves the next ticket number of a project.
    pub fn take_ticket_number(&self, project: ProjectId) -> Result<u32, StoreError> {
        let n: u32 = self.conn.query_row(
            "UPDATE projects SET next_ticket_number = next_ticket_number + 1 WHERE id = ?1 RETURNING next_ticket_number - 1",
            [project.to_string()],
            |r| r.get(0),
        )?;
        Ok(n)
    }

    // ---- chains ----

    pub fn insert_chain(&self, c: &Chain) -> Result<(), StoreError> {
        self.conn.execute(
            &format!("INSERT INTO chains ({CHAIN_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
            params![
                c.id.to_string(),
                c.project_id.to_string(),
                c.name,
                c.goal,
                c.autostart,
                opt_enum(&c.review_mode)?
            ],
        )?;
        Ok(())
    }

    pub fn update_chain(&self, c: &Chain) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE chains SET name = ?2, goal = ?3, autostart = ?4, review_mode = ?5 WHERE id = ?1",
            params![c.id.to_string(), c.name, c.goal, c.autostart, opt_enum(&c.review_mode)?],
        )?;
        Ok(())
    }

    fn chain_from(
        r: &Row<'_>,
    ) -> rusqlite::Result<(String, String, String, String, bool, Option<String>)> {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
        ))
    }

    fn chain_build(
        t: (String, String, String, String, bool, Option<String>),
    ) -> Result<Chain, StoreError> {
        Ok(Chain {
            id: parse(&t.0, "chain id")?,
            project_id: parse(&t.1, "project id")?,
            name: t.2,
            goal: t.3,
            autostart: t.4,
            review_mode: opt_enum_from(t.5)?,
        })
    }

    pub fn chain(&self, id: ChainId) -> Result<Option<Chain>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {CHAIN_COLS} FROM chains WHERE id = ?1"),
                [id.to_string()],
                Self::chain_from,
            )
            .optional()?
            .map(Self::chain_build)
            .transpose()
    }

    pub fn chain_by_name(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<Option<Chain>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {CHAIN_COLS} FROM chains WHERE project_id = ?1 AND name = ?2"),
                params![project.to_string(), name],
                Self::chain_from,
            )
            .optional()?
            .map(Self::chain_build)
            .transpose()
    }

    pub fn chains(&self, project: Option<ProjectId>) -> Result<Vec<Chain>, StoreError> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {CHAIN_COLS} FROM chains WHERE ?1 IS NULL OR project_id = ?1 ORDER BY rowid"
        ))?;
        let rows = st.query_map([project.map(|p| p.to_string())], Self::chain_from)?;
        rows.map(|r| Self::chain_build(r?)).collect()
    }

    /// Chains with this name, across projects.
    pub fn chains_named(&self, name: &str) -> Result<Vec<Chain>, StoreError> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {CHAIN_COLS} FROM chains WHERE name = ?1 ORDER BY rowid"
        ))?;
        let rows = st.query_map([name], Self::chain_from)?;
        rows.map(|r| Self::chain_build(r?)).collect()
    }

    // ---- releases, edges ----

    pub fn upsert_release(&self, r: &Release) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO releases ({RELEASE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(id) DO UPDATE SET version = excluded.version, channel = excluded.channel, \
                 start_mode = excluded.start_mode, state = excluded.state, branch = excluded.branch, \
                 cut_at = excluded.cut_at, released_at = excluded.released_at"
            ),
            params![
                r.id.to_string(),
                r.chain_id.to_string(),
                r.version,
                enum_str(&r.channel)?,
                enum_str(&r.start_mode)?,
                enum_str(&r.state)?,
                r.branch,
                r.cut_at.map(|t| t.0),
                r.released_at.map(|t| t.0),
            ],
        )?;
        Ok(())
    }

    fn releases(&self, chain: ChainId) -> Result<Vec<Release>, StoreError> {
        type Raw = (
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<i64>,
        );
        let mut st = self.conn.prepare(&format!(
            "SELECT {RELEASE_COLS} FROM releases WHERE chain_id = ?1 ORDER BY rowid"
        ))?;
        let rows = st.query_map([chain.to_string()], |r| -> rusqlite::Result<Raw> {
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
            ))
        })?;
        rows.map(|r| {
            let r = r?;
            Ok(Release {
                id: parse(&r.0, "release id")?,
                chain_id: parse(&r.1, "chain id")?,
                version: r.2,
                channel: enum_from(&r.3)?,
                start_mode: enum_from(&r.4)?,
                state: enum_from(&r.5)?,
                branch: r.6,
                cut_at: r.7.map(Timestamp),
                released_at: r.8.map(Timestamp),
            })
        })
        .collect()
    }

    pub fn release(&self, id: ReleaseId) -> Result<Option<Release>, StoreError> {
        let chain: Option<String> = self
            .conn
            .query_row(
                "SELECT chain_id FROM releases WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let Some(chain) = chain else { return Ok(None) };
        Ok(self
            .releases(parse(&chain, "chain id")?)?
            .into_iter()
            .find(|r| r.id == id))
    }

    /// Releases with this version, across chains.
    pub fn releases_by_version(&self, version: &str) -> Result<Vec<Release>, StoreError> {
        let mut st = self
            .conn
            .prepare("SELECT chain_id FROM releases WHERE version = ?1 ORDER BY rowid")?;
        let chains = st
            .query_map([version], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = Vec::new();
        for c in chains {
            out.extend(
                self.releases(parse(&c, "chain id")?)?
                    .into_iter()
                    .filter(|r| r.version == version),
            );
        }
        Ok(out)
    }

    /// Everything the graph rules need about one chain.
    pub fn chain_view(&self, chain: ChainId) -> Result<Option<ChainView>, StoreError> {
        let Some(c) = self.chain(chain)? else {
            return Ok(None);
        };
        let mut tickets = self.tickets_where("chain_id = ?1", chain.to_string())?;
        tickets.sort_by_key(|t| t.key.number());
        let mut st = self.conn.prepare(
            "SELECT from_kind, from_id, to_kind, to_id, kind FROM edges WHERE chain_id = ?1 ORDER BY rowid",
        )?;
        let edges = st
            .query_map([chain.to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .map(|r| {
                let (fk, fi, tk, ti, kind) = r?;
                Ok(Edge {
                    chain_id: chain,
                    from: node_from(&fk, &fi)?,
                    to: node_from(&tk, &ti)?,
                    kind: enum_from(&kind)?,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let mut st = self.conn.prepare(
            "SELECT rt.release_id, rt.ticket_id FROM release_tickets rt JOIN releases r ON r.id = rt.release_id \
             WHERE r.chain_id = ?1 ORDER BY rt.rowid",
        )?;
        let release_tickets = st
            .query_map([chain.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .map(|r| {
                let (rel, t) = r?;
                Ok((parse(&rel, "release id")?, parse(&t, "ticket id")?))
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok(Some(ChainView {
            chain: c,
            tickets,
            releases: self.releases(chain)?,
            edges,
            release_tickets,
        }))
    }

    /// Writes a chain view back: upserts tickets and releases, replaces edges and memberships.
    pub fn save_chain_view(&self, v: &ChainView) -> Result<(), StoreError> {
        self.update_chain(&v.chain)?;
        for t in &v.tickets {
            self.upsert_ticket(t)?;
        }
        for r in &v.releases {
            self.upsert_release(r)?;
        }
        let chain = v.chain.id.to_string();
        self.conn
            .execute("DELETE FROM edges WHERE chain_id = ?1", [&chain])?;
        for e in &v.edges {
            let (fk, fi) = node_parts(&e.from);
            let (tk, ti) = node_parts(&e.to);
            self.conn.execute(
                "INSERT INTO edges (chain_id, from_kind, from_id, to_kind, to_id, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![chain, fk, fi, tk, ti, enum_str(&e.kind)?],
            )?;
        }
        self.conn.execute(
            "DELETE FROM release_tickets WHERE release_id IN (SELECT id FROM releases WHERE chain_id = ?1)",
            [&chain],
        )?;
        for (r, t) in &v.release_tickets {
            self.conn.execute(
                "INSERT INTO release_tickets (release_id, ticket_id) VALUES (?1, ?2)",
                params![r.to_string(), t.to_string()],
            )?;
        }
        Ok(())
    }

    // ---- events ----

    pub fn events_since(&self, since: i64, limit: usize) -> Result<Vec<EventRecord>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT seq, at, actor, entity, entity_id, kind, payload_json FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )?;
        let rows = st.query_map(
            params![since, i64::try_from(limit).unwrap_or(i64::MAX)],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            },
        )?;
        rows.map(|r| {
            let r = r?;
            Ok(EventRecord {
                seq: r.0,
                at: Timestamp(r.1),
                actor: enum_from(&r.2)?,
                entity: r.3,
                entity_id: r.4,
                kind: r.5,
                payload: from_json(&r.6)?,
            })
        })
        .collect()
    }

    /// Payload of the newest journal event about one entity, if any.
    pub fn last_event_payload(
        &self,
        entity: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        self.conn
            .query_row(
                "SELECT payload_json FROM events WHERE entity = ?1 AND entity_id = ?2 ORDER BY seq DESC LIMIT 1",
                params![entity, id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| from_json(&s))
            .transpose()
    }

    /// Runs SQLite's own consistency check; `ok` means the file is sound.
    pub fn integrity_check(&self) -> Result<String, StoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?)
    }

    pub fn last_seq(&self) -> Result<i64, StoreError> {
        super::last_seq(self.conn)
    }
}
