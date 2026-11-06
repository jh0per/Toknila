//! Row <-> domain mapping for projects.

use rusqlite::{OptionalExtension, Row, params};
use toknila_core::{Project, ProjectId};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, parse};

const PROJECT_COLS: &str = "id, key, name, repo_path, work_branch, release_branch, default_profile, \
     default_review_mode, max_parallel, default_reviewer";

type RawProject = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    i64,
    Option<String>,
);

fn project_row(r: &Row<'_>) -> rusqlite::Result<RawProject> {
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
    ))
}

fn project_from(c: RawProject) -> Result<Project, StoreError> {
    let (
        id,
        key,
        name,
        repo_path,
        work_branch,
        release_branch,
        default_profile,
        review,
        max,
        reviewer,
    ) = c;
    Ok(Project {
        id: parse(&id, "project id")?,
        key: parse(&key, "project key")?,
        name,
        repo_path,
        work_branch,
        release_branch,
        default_profile,
        default_review_mode: enum_from(&review)?,
        max_parallel: u32::try_from(max)
            .map_err(|_| StoreError::Corrupt(format!("bad max_parallel {max}")))?,
        default_reviewer: reviewer,
    })
}

impl Repo<'_> {
    pub fn insert_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO projects ({PROJECT_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
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
                p.max_parallel,
                p.default_reviewer,
            ],
        )?;
        Ok(())
    }

    /// Saves the settings of an existing project (its key and id never change).
    pub fn update_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE projects SET name = ?2, repo_path = ?3, work_branch = ?4, release_branch = ?5, \
             default_profile = ?6, default_review_mode = ?7, max_parallel = ?8, default_reviewer = ?9 \
             WHERE id = ?1",
            params![
                p.id.to_string(),
                p.name,
                p.repo_path,
                p.work_branch,
                p.release_branch,
                p.default_profile,
                enum_str(&p.default_review_mode)?,
                p.max_parallel,
                p.default_reviewer,
            ],
        )?;
        Ok(())
    }

    pub fn projects(&self) -> Result<Vec<Project>, StoreError> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {PROJECT_COLS} FROM projects ORDER BY key"))?;
        let rows = st.query_map([], project_row)?;
        rows.map(|r| project_from(r?)).collect()
    }

    pub fn project(&self, id: ProjectId) -> Result<Option<Project>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1"),
                [id.to_string()],
                project_row,
            )
            .optional()?
            .map(project_from)
            .transpose()
    }

    pub fn project_by_key(&self, key: &str) -> Result<Option<Project>, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE key = ?1"),
                [key],
                project_row,
            )
            .optional()?
            .map(project_from)
            .transpose()
    }
}
