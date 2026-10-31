//! Row <-> domain mapping. Enums are stored as their serde names; nested values as JSON.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use toknila_core::graph::ChainView;
use toknila_core::{
    Chain, ChainId, Comment, CommentId, Edge, NodeRef, Project, ProjectId, Release, ReleaseId,
    Subtask, Ticket, TicketId, TicketKey, Timestamp,
};

use super::{EventRecord, StoreError};

/// Serde name of a unit enum value (`TicketStatus::Running` -> `"running"`).
pub(crate) fn enum_str<T: Serialize>(value: &T) -> Result<String, StoreError> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(StoreError::Corrupt(format!(
            "expected a unit enum, got {other}"
        ))),
    }
}

fn enum_from<T: DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        s.to_owned(),
    ))?)
}

fn json<T: Serialize>(v: &T) -> Result<String, StoreError> {
    Ok(serde_json::to_string(v)?)
}

fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_str(s)?)
}

fn parse<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, StoreError> {
    s.parse()
        .map_err(|_| StoreError::Corrupt(format!("bad {what}: {s:?}")))
}

fn opt_enum<T: Serialize>(v: &Option<T>) -> Result<Option<String>, StoreError> {
    v.as_ref().map(enum_str).transpose()
}

fn opt_enum_from<T: DeserializeOwned>(v: Option<String>) -> Result<Option<T>, StoreError> {
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

const PROJECT_COLS: &str =
    "id, key, name, repo_path, work_branch, release_branch, default_profile, default_review_mode";
const CHAIN_COLS: &str = "id, project_id, name, goal, autostart, review_mode";
const TICKET_COLS: &str = "id, chain_id, key, title, prompt, acceptance_json, status, status_reason_json, \
     status_since, paused, profile, review_mode, branch, needs_secrets, tokens_used";
const RELEASE_COLS: &str =
    "id, chain_id, version, channel, start_mode, state, branch, cut_at, released_at";

impl<'c> Repo<'c> {
    pub(crate) fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    // ---- projects ----

    pub fn insert_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO projects ({PROJECT_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                p.id.to_string(),
                p.key.as_str(),
                p.name,
                p.repo_path,
                p.work_branch,
                p.release_branch,
                p.default_profile,
                enum_str(&p.default_review_mode)?
            ],
        )?;
        Ok(())
    }

    fn project_row(r: &Row<'_>) -> rusqlite::Result<[Option<String>; 8]> {
        Ok([
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
        ])
    }

    fn project_from(c: [Option<String>; 8]) -> Result<Project, StoreError> {
        let [id, key, name, repo_path, work, release, profile, review] = c;
        let req =
            |v: Option<String>| v.ok_or_else(|| StoreError::Corrupt("null project column".into()));
        Ok(Project {
            id: parse(&req(id)?, "project id")?,
            key: parse(&req(key)?, "project key")?,
            name: req(name)?,
            repo_path,
            work_branch: req(work)?,
            release_branch: req(release)?,
            default_profile: req(profile)?,
            default_review_mode: enum_from(&req(review)?)?,
        })
    }

    pub fn projects(&self) -> Result<Vec<Project>, StoreError> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {PROJECT_COLS} FROM projects ORDER BY key"))?;
        let rows = st.query_map([], Self::project_row)?;
        rows.map(|r| Self::project_from(r?)).collect()
    }

    pub fn project(&self, id: ProjectId) -> Result<Option<Project>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                [id.to_string()],
                Self::project_row,
            )
            .optional()?
            .map(Self::project_from)
            .transpose()
    }

    pub fn project_by_key(&self, key: &str) -> Result<Option<Project>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE key = ?1"),
                [key],
                Self::project_row,
            )
            .optional()?
            .map(Self::project_from)
            .transpose()
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

    // ---- tickets ----

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
                "INSERT INTO tickets ({TICKET_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15) \
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, prompt = excluded.prompt, \
                 acceptance_json = excluded.acceptance_json, status = excluded.status, \
                 status_reason_json = excluded.status_reason_json, status_since = excluded.status_since, \
                 paused = excluded.paused, profile = excluded.profile, review_mode = excluded.review_mode, \
                 branch = excluded.branch, needs_secrets = excluded.needs_secrets, tokens_used = excluded.tokens_used"
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
            ],
        )?;
        Ok(())
    }

    fn tickets_where(&self, clause: &str, arg: String) -> Result<Vec<Ticket>, StoreError> {
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
                profile: r.10,
                review_mode: opt_enum_from(r.11)?,
                branch: r.12,
                needs_secrets: r.13,
                tokens_used: u64::try_from(r.14).unwrap_or(0),
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

    // ---- subtasks ----

    pub fn upsert_subtask(&self, s: &Subtask) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO subtasks (id, ticket_id, position, text, done, done_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(id) DO UPDATE SET position = excluded.position, text = excluded.text, \
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

    pub fn last_seq(&self) -> Result<i64, StoreError> {
        super::last_seq(self.conn)
    }
}
