//! Row <-> domain mapping for projects and chains.

use rusqlite::{OptionalExtension, Row, params};
use toknila_core::{Chain, ChainId, Project, ProjectId};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, opt_enum, opt_enum_from, parse};

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

const PROJECT_COLS: &str = "id, key, name, repo_path, work_branch, release_branch, default_profile, \
     default_review_mode, default_reviewer";
const CHAIN_COLS: &str = "id, project_id, name, goal, autostart, review_mode, reviewer";
impl Repo<'_> {
    // ---- projects ----

    pub fn insert_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO projects ({PROJECT_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            params![
                p.id.to_string(),
                p.key.as_str(),
                p.name,
                p.repo_path,
                p.work_branch,
                p.release_branch,
                p.default_profile,
                enum_str(&p.default_review_mode)?,
                p.default_reviewer
            ],
        )?;
        Ok(())
    }

    /// Writes a project's settings back; its id and key stay.
    pub fn update_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE projects SET name = ?2, repo_path = ?3, work_branch = ?4, release_branch = ?5, \
             default_profile = ?6, default_review_mode = ?7, default_reviewer = ?8 WHERE id = ?1",
            params![
                p.id.to_string(),
                p.name,
                p.repo_path,
                p.work_branch,
                p.release_branch,
                p.default_profile,
                enum_str(&p.default_review_mode)?,
                p.default_reviewer
            ],
        )?;
        Ok(())
    }

    fn project_row(r: &Row<'_>) -> rusqlite::Result<[Option<String>; 9]> {
        Ok([
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
        ])
    }

    fn project_from(c: [Option<String>; 9]) -> Result<Project, StoreError> {
        let [
            id,
            key,
            name,
            repo_path,
            work,
            release,
            profile,
            review,
            reviewer,
        ] = c;
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
            default_reviewer: reviewer,
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
