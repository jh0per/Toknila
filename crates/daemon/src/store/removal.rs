//! Deleting a project's or a chain's rows: everything that hangs off it, child tables first so
//! the foreign keys hold, and the journal entries about any of it. Runs inside the command's
//! transaction; the events that tell of the deletion are journaled after it.

use rusqlite::params;
use toknila_core::{ChainId, ProjectId};

use super::StoreError;
use super::codec::Repo;

/// What a deletion covers, as subqueries over `?1`: the chains, and from them the cards and
/// milestones. A project deletion selects the chains of the project, a chain deletion the one.
struct Scope {
    chains: &'static str,
    tickets: String,
    releases: String,
}

impl Scope {
    fn new(chains: &'static str) -> Self {
        Self {
            chains,
            tickets: format!("SELECT id FROM tickets WHERE chain_id IN ({chains})"),
            releases: format!("SELECT id FROM releases WHERE chain_id IN ({chains})"),
        }
    }

    fn under_tickets(&self, table: &str) -> String {
        format!(
            "SELECT id FROM {table} WHERE ticket_id IN ({})",
            self.tickets
        )
    }

    /// The statements, ending with the chain rows. `own` is the journal condition about the
    /// owner of the chains (the project), if the deletion covers it.
    fn statements(&self, own: Option<&str>) -> Vec<String> {
        let (chains, tickets, releases) = (self.chains, &self.tickets, &self.releases);
        let (threads, ships) = (self.under_tickets("threads"), self.under_tickets("ships"));
        let own = own.map(|o| format!("{o} OR ")).unwrap_or_default();
        let journal = format!(
            "DELETE FROM events WHERE {own}(entity = 'chain' AND entity_id IN ({chains})) \
             OR (entity = 'ticket' AND entity_id IN ({tickets})) \
             OR (entity = 'release' AND entity_id IN ({releases})) \
             OR (entity = 'thread' AND entity_id IN ({threads})) \
             OR (entity = 'comment' AND entity_id IN ({})) \
             OR (entity = 'subtask' AND entity_id IN ({}))",
            self.under_tickets("comments"),
            self.under_tickets("subtasks"),
        );
        vec![
            journal,
            format!("DELETE FROM runners WHERE ship_id IN ({ships})"),
            format!("DELETE FROM ships WHERE ticket_id IN ({tickets})"),
            format!("DELETE FROM usage_sessions WHERE ticket_id IN ({tickets})"),
            format!("UPDATE usage_records SET ticket_id = NULL WHERE ticket_id IN ({tickets})"),
            format!("DELETE FROM thread_messages WHERE thread_id IN ({threads})"),
            format!("DELETE FROM thread_versions WHERE thread_id IN ({threads})"),
            format!("DELETE FROM threads WHERE ticket_id IN ({tickets})"),
            format!("DELETE FROM comments WHERE ticket_id IN ({tickets})"),
            format!("DELETE FROM subtasks WHERE ticket_id IN ({tickets})"),
            format!(
                "DELETE FROM release_tickets WHERE release_id IN ({releases}) OR ticket_id IN ({tickets})"
            ),
            format!("DELETE FROM edges WHERE chain_id IN ({chains})"),
            format!("DELETE FROM releases WHERE chain_id IN ({chains})"),
            format!("DELETE FROM tickets WHERE chain_id IN ({chains})"),
            format!("DELETE FROM chains WHERE id IN ({chains})"),
        ]
    }
}

impl Repo<'_> {
    /// Deletes the project and every row that belongs to it, with the journal entries about the
    /// project, its chains, cards, milestones, comments, subtasks and threads. Token usage
    /// records stay, no longer tied to a card; the read offsets of the transcripts stay too, so
    /// they are not read again.
    pub fn delete_project(&self, project: ProjectId) -> Result<(), StoreError> {
        let scope = Scope::new("SELECT id FROM chains WHERE project_id = ?1");
        let mut sql = scope.statements(Some("(entity = 'project' AND entity_id = ?1)"));
        sql.push("DELETE FROM projects WHERE id = ?1".to_owned());
        self.run(&sql, &project.to_string())
    }

    /// Deletes one chain the same way: its cards, milestones, edges and everything attached to
    /// them, with their journal entries. The project and its other chains stay, and so does the
    /// project's ticket counter, so no card key is handed out twice.
    pub fn delete_chain(&self, chain: ChainId) -> Result<(), StoreError> {
        let scope = Scope::new("SELECT id FROM chains WHERE id = ?1");
        self.run(&scope.statements(None), &chain.to_string())
    }

    fn run(&self, statements: &[String], id: &str) -> Result<(), StoreError> {
        for sql in statements {
            self.conn.execute(sql, params![id])?;
        }
        Ok(())
    }
}
