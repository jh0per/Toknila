//! Row <-> domain mapping for ships. One row per ticket: its id is the ticket's id, so a ticket
//! never has two, and the ticket's key comes from a join.

use rusqlite::{OptionalExtension, params};
use toknila_core::{Ship, TicketId};

use super::StoreError;
use super::codec::{Repo, parse};

impl Repo<'_> {
    pub fn save_ship(&self, s: &Ship) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO ships (id, ticket_id, worktree_path, branch, state) \
             VALUES (?1, ?1, ?2, ?3, 'ready') \
             ON CONFLICT(id) DO UPDATE SET worktree_path = excluded.worktree_path, \
             branch = excluded.branch",
            params![s.ticket_id.to_string(), s.path, s.branch],
        )?;
        Ok(())
    }

    pub fn delete_ship(&self, ticket: TicketId) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM ships WHERE id = ?1", [ticket.to_string()])?;
        Ok(())
    }

    pub fn ship(&self, ticket: TicketId) -> Result<Option<Ship>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT t.key, s.branch, s.worktree_path FROM ships s \
                 JOIN tickets t ON t.id = s.ticket_id WHERE s.id = ?1",
                [ticket.to_string()],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(key, branch, path)| {
            Ok(Ship {
                ticket_id: ticket,
                key: parse(&key, "ticket key")?,
                branch,
                path,
            })
        })
        .transpose()
    }
}
