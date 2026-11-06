//! Row <-> domain mapping for chains.

use rusqlite::{OptionalExtension, Row, params};
use toknila_core::{Chain, ChainId, ProjectId};

use super::StoreError;
use super::codec::{Repo, opt_enum, opt_enum_from, parse};

/// A chain row: id, project, name, goal, autostart, review mode and reviewer.
type ChainRow = (
    String,
    String,
    String,
    String,
    bool,
    Option<String>,
    Option<String>,
);

const CHAIN_COLS: &str = "id, project_id, name, goal, autostart, review_mode, reviewer";

impl Repo<'_> {
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
            &format!("INSERT INTO chains ({CHAIN_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"),
            params![
                c.id.to_string(),
                c.project_id.to_string(),
                c.name,
                c.goal,
                c.autostart,
                opt_enum(&c.review_mode)?,
                c.reviewer
            ],
        )?;
        Ok(())
    }

    pub fn update_chain(&self, c: &Chain) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE chains SET name = ?2, goal = ?3, autostart = ?4, review_mode = ?5, reviewer = ?6 WHERE id = ?1",
            params![
                c.id.to_string(),
                c.name,
                c.goal,
                c.autostart,
                opt_enum(&c.review_mode)?,
                c.reviewer
            ],
        )?;
        Ok(())
    }

    fn chain_from(r: &Row<'_>) -> rusqlite::Result<ChainRow> {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
        ))
    }

    fn chain_build(t: ChainRow) -> Result<Chain, StoreError> {
        Ok(Chain {
            id: parse(&t.0, "chain id")?,
            project_id: parse(&t.1, "project id")?,
            name: t.2,
            goal: t.3,
            autostart: t.4,
            review_mode: opt_enum_from(t.5)?,
            reviewer: t.6,
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
}
