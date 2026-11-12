//! Row <-> domain mapping for projects.

use rusqlite::{OptionalExtension, Row, params};
use toknila_core::{MergeSettings, Project, ProjectId};

use super::StoreError;
use super::codec::{Repo, enum_from, enum_str, parse};

const PROJECT_COLS: &str = "id, key, name, repo_path, work_branch, release_branch, default_profile, \
     default_review_mode, max_parallel, default_reviewer, merge_mode, pr_title, merge_method, check_command, archived, \
     worker_model, reviewer_model, planner_model, command_policy, allowed_commands, reviewer_web_domains, max_review_rounds, \
     no_checks_timeout_secs, bump_command, setup_command";

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
    String,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
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
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
        r.get(13)?,
        r.get(14)?,
        r.get(15)?,
        r.get(16)?,
        r.get(17)?,
        r.get(18)?,
        r.get(19)?,
        r.get(20)?,
        r.get(21)?,
        r.get(22)?,
        r.get(23)?,
        r.get(24)?,
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
        merge_mode,
        pr_title,
        merge_method,
        check_command,
        archived,
        worker_model,
        reviewer_model,
        planner_model,
        command_policy,
        allowed_commands,
        reviewer_web_domains,
        max_review_rounds,
        no_checks_timeout_secs,
        bump_command,
        setup_command,
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
        max_review_rounds: u32::try_from(max_review_rounds).map_err(|_| {
            StoreError::Corrupt(format!("bad max_review_rounds {max_review_rounds}"))
        })?,
        merge: MergeSettings {
            mode: enum_from(&merge_mode)?,
            pr_title,
            method: enum_from(&merge_method)?,
            no_checks_timeout_secs: u64::try_from(no_checks_timeout_secs).map_err(|_| {
                StoreError::Corrupt(format!(
                    "bad no_checks_timeout_secs {no_checks_timeout_secs}"
                ))
            })?,
        },
        check_command,
        setup_command,
        bump_command,
        worker_model,
        reviewer_model,
        planner_model,
        command_policy: enum_from(&command_policy)?,
        allowed_commands: serde_json::from_str(&allowed_commands)
            .map_err(|e| StoreError::Corrupt(format!("bad allowed_commands: {e}")))?,
        reviewer_web_domains: serde_json::from_str(&reviewer_web_domains)
            .map_err(|e| StoreError::Corrupt(format!("bad reviewer_web_domains: {e}")))?,
        archived,
    })
}

impl Repo<'_> {
    pub fn insert_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            &format!(
                "INSERT INTO projects ({PROJECT_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)"
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
                enum_str(&p.merge.mode)?,
                p.merge.pr_title,
                enum_str(&p.merge.method)?,
                p.check_command,
                p.archived,
                p.worker_model,
                p.reviewer_model,
                p.planner_model,
                enum_str(&p.command_policy)?,
                serde_json::to_string(&p.allowed_commands)?,
                serde_json::to_string(&p.reviewer_web_domains)?,
                p.max_review_rounds,
                i64::try_from(p.merge.no_checks_timeout_secs).unwrap_or(i64::MAX),
                p.bump_command,
                p.setup_command,
            ],
        )?;
        Ok(())
    }

    /// Saves the settings of an existing project (its key and id never change).
    pub fn update_project(&self, p: &Project) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE projects SET name = ?2, repo_path = ?3, work_branch = ?4, release_branch = ?5, \
             default_profile = ?6, default_review_mode = ?7, max_parallel = ?8, default_reviewer = ?9, \
             merge_mode = ?10, pr_title = ?11, merge_method = ?12, check_command = ?13, archived = ?14, \
             worker_model = ?15, reviewer_model = ?16, planner_model = ?17, command_policy = ?18, allowed_commands = ?19, reviewer_web_domains = ?20, \
             max_review_rounds = ?21, no_checks_timeout_secs = ?22, bump_command = ?23, setup_command = ?24 WHERE id = ?1",
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
                enum_str(&p.merge.mode)?,
                p.merge.pr_title,
                enum_str(&p.merge.method)?,
                p.check_command,
                p.archived,
                p.worker_model,
                p.reviewer_model,
                p.planner_model,
                enum_str(&p.command_policy)?,
                serde_json::to_string(&p.allowed_commands)?,
                serde_json::to_string(&p.reviewer_web_domains)?,
                p.max_review_rounds,
                i64::try_from(p.merge.no_checks_timeout_secs).unwrap_or(i64::MAX),
                p.bump_command,
                p.setup_command,
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
