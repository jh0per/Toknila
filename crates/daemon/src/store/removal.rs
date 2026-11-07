//! Deleting a project's rows: everything that hangs off the project, child tables first so the
//! foreign keys hold, and the journal entries about any of it. Runs inside the command's
//! transaction; the events that tell of the deletion are journaled after it.

use rusqlite::params;
use toknila_core::ProjectId;

use super::StoreError;
use super::codec::Repo;

/// The project's chains, cards, milestones, threads and ships, as subqueries over `?1`.
const CHAINS: &str = "SELECT id FROM chains WHERE project_id = ?1";
const TICKETS: &str =
    "SELECT id FROM tickets WHERE chain_id IN (SELECT id FROM chains WHERE project_id = ?1)";
const RELEASES: &str =
    "SELECT id FROM releases WHERE chain_id IN (SELECT id FROM chains WHERE project_id = ?1)";

fn under_tickets(table: &str) -> String {
    format!("SELECT id FROM {table} WHERE ticket_id IN ({TICKETS})")
}

impl Repo<'_> {
    /// Deletes the project and every row that belongs to it, with the journal entries about the
    /// project, its chains, cards, milestones, comments, subtasks and threads. Token usage
    /// records stay, no longer tied to a card; the read offsets of the transcripts stay too, so
    /// they are not read again.
    pub fn delete_project(&self, project: ProjectId) -> Result<(), StoreError> {
        let (threads, ships) = (under_tickets("threads"), under_tickets("ships"));
        let journal = format!(
            "DELETE FROM events WHERE (entity = 'project' AND entity_id = ?1) \
             OR (entity = 'chain' AND entity_id IN ({CHAINS})) \
             OR (entity = 'ticket' AND entity_id IN ({TICKETS})) \
             OR (entity = 'release' AND entity_id IN ({RELEASES})) \
             OR (entity = 'thread' AND entity_id IN ({threads})) \
             OR (entity = 'comment' AND entity_id IN ({})) \
             OR (entity = 'subtask' AND entity_id IN ({}))",
            under_tickets("comments"),
            under_tickets("subtasks"),
        );
        let statements = [
            journal,
            format!("DELETE FROM runners WHERE ship_id IN ({ships})"),
            format!("DELETE FROM ships WHERE ticket_id IN ({TICKETS})"),
            format!("DELETE FROM usage_sessions WHERE ticket_id IN ({TICKETS})"),
            format!("UPDATE usage_records SET ticket_id = NULL WHERE ticket_id IN ({TICKETS})"),
            format!("DELETE FROM thread_messages WHERE thread_id IN ({threads})"),
            format!("DELETE FROM thread_versions WHERE thread_id IN ({threads})"),
            format!("DELETE FROM threads WHERE ticket_id IN ({TICKETS})"),
            format!("DELETE FROM comments WHERE ticket_id IN ({TICKETS})"),
            format!("DELETE FROM subtasks WHERE ticket_id IN ({TICKETS})"),
            format!(
                "DELETE FROM release_tickets WHERE release_id IN ({RELEASES}) OR ticket_id IN ({TICKETS})"
            ),
            format!("DELETE FROM edges WHERE chain_id IN ({CHAINS})"),
            format!("DELETE FROM releases WHERE chain_id IN ({CHAINS})"),
            format!("DELETE FROM tickets WHERE chain_id IN ({CHAINS})"),
            "DELETE FROM chains WHERE project_id = ?1".to_owned(),
            "DELETE FROM projects WHERE id = ?1".to_owned(),
        ];
        let id = project.to_string();
        for sql in &statements {
            self.conn.execute(sql, params![id])?;
        }
        Ok(())
    }
}
