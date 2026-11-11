//! Row <-> domain mapping for the assistant's pins on designs.

use rusqlite::{OptionalExtension, params};
use toknila_core::design::pin::DesignPin;
use toknila_core::{PinId, ProjectId, Timestamp};

use super::StoreError;
use super::codec::{Repo, from_json, json, parse};

const COLUMNS: &str = "id, project_id, design_id, design_name, number, region_json, text, card, \
                       answer, created_at, answered_at";

type Row = (
    String,
    String,
    String,
    String,
    u32,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<i64>,
);

fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
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
    ))
}

fn pin(r: Row) -> Result<DesignPin, StoreError> {
    Ok(DesignPin {
        id: parse(&r.0, "pin id")?,
        project: parse(&r.1, "project id")?,
        design: parse(&r.2, "design id")?,
        design_name: r.3,
        number: r.4,
        region: from_json(&r.5)?,
        text: r.6,
        card: r.7.as_deref().map(|k| parse(k, "ticket key")).transpose()?,
        answer: r.8,
        created_at: Timestamp(r.9),
        answered_at: r.10.map(Timestamp),
    })
}

impl Repo<'_> {
    /// Inserts a pin, or updates its answer.
    pub fn upsert_design_pin(&self, p: &DesignPin) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO design_pins (id, project_id, design_id, design_name, number, region_json, \
             text, card, answer, created_at, answered_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
             ?9, ?10, ?11) ON CONFLICT(id) DO UPDATE SET answer = excluded.answer, \
             answered_at = excluded.answered_at",
            params![
                p.id.to_string(),
                p.project.to_string(),
                p.design.to_string(),
                p.design_name,
                p.number,
                json(&p.region)?,
                p.text,
                p.card.as_ref().map(ToString::to_string),
                p.answer,
                p.created_at.0,
                p.answered_at.map(|a| a.0),
            ],
        )?;
        Ok(())
    }

    /// Forgets a pin.
    pub fn delete_design_pin(&self, id: PinId) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM design_pins WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }

    /// The pins of a project, oldest first.
    pub fn design_pins(&self, project: ProjectId) -> Result<Vec<DesignPin>, StoreError> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM design_pins WHERE project_id = ?1 ORDER BY created_at, rowid"
        ))?;
        let rows = st.query_map([project.to_string()], read)?;
        rows.map(|r| pin(r?)).collect()
    }

    /// One pin by id.
    pub fn design_pin(&self, id: PinId) -> Result<Option<DesignPin>, StoreError> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM design_pins WHERE id = ?1"))?;
        st.query_row([id.to_string()], read)
            .optional()?
            .map(pin)
            .transpose()
    }
}
