//! The planner's standing suggestion: one row per project, replaced by the next one and removed
//! when core says an event ended it.

use rusqlite::{OptionalExtension, params};
use toknila_core::suggestion::{Ends, Suggestion};
use toknila_core::{ProjectId, ProjectKey};

use super::StoreError;
use super::codec::{Repo, from_json, json, parse};

impl Repo<'_> {
    /// The project's standing suggestion, if it has one.
    pub fn suggestion(&self, project: ProjectId) -> Result<Option<Suggestion>, StoreError> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM suggestions WHERE project_id = ?1",
                [project.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        text.map(|t| from_json(&t)).transpose()
    }

    /// Keeps `s` as the project's suggestion, replacing the one it had.
    pub fn save_suggestion(&self, project: ProjectId, s: &Suggestion) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO suggestions (project_id, card, chain_id, action, json) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                project.to_string(),
                s.card.as_ref().map(ToString::to_string),
                s.chain.map(|c| c.to_string()),
                s.action.map(|a| a.as_str()),
                json(s)?,
            ],
        )?;
        Ok(())
    }

    /// Removes the project's suggestion; whether there was one.
    pub fn clear_suggestion(&self, project: ProjectId) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "DELETE FROM suggestions WHERE project_id = ?1",
            [project.to_string()],
        )?;
        Ok(n > 0)
    }

    /// Removes the suggestions that `ends` names; the projects they belonged to.
    pub fn end_suggestions(&self, ends: &Ends) -> Result<Vec<(ProjectId, ProjectKey)>, StoreError> {
        let (sql, arg) = match ends {
            Ends::Card(key) => (
                "SELECT project_id, json FROM suggestions WHERE card = ?1",
                key.to_string(),
            ),
            Ends::Release(chain) => (
                "SELECT project_id, json FROM suggestions WHERE action = 'release' AND chain_id = ?1",
                chain.to_string(),
            ),
        };
        let mut st = self.conn.prepare(sql)?;
        let rows = st
            .query_map([arg], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut ended = Vec::new();
        for (id, text) in rows {
            let id: ProjectId = parse(&id, "project id")?;
            let s: Suggestion = from_json(&text)?;
            self.clear_suggestion(id)?;
            ended.push((id, s.project));
        }
        Ok(ended)
    }
}
