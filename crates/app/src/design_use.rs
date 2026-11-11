//! Who uses a design of the library: the cards that reference it and the open pins on it. A
//! design in use is not removed unless the person forces it; forcing takes the references and the
//! open pins away in the same unit of work and tells each card which design went.

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::{Actor, CommentBody, CommentKind, DesignId, ProjectId, TicketKey};

use crate::inbox::project_tickets;
use crate::notes::add_comment;
use crate::{AppError, AppResult, Reader, UnitOfWork, with_chain};

/// What still uses a design.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct DesignUse {
    /// The cards that reference the design, in chain order.
    pub cards: Vec<TicketKey>,
    /// How many open pins are on the design.
    pub pins: u32,
}

impl DesignUse {
    /// Whether nothing uses the design.
    pub fn is_free(&self) -> bool {
        self.cards.is_empty() && self.pins == 0
    }

    /// The refusal text: `<name> is used by TOKNILA-12, TOKNILA-14 and 2 pins.`
    pub fn sentence(&self, name: &str) -> String {
        let mut parts: Vec<String> = self.cards.iter().map(ToString::to_string).collect();
        if self.pins > 0 {
            let s = if self.pins == 1 { "" } else { "s" };
            parts.push(format!("{} pin{s}", self.pins));
        }
        let used = match parts.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            Some((last, _)) => last.clone(),
            None => "nothing".to_owned(),
        };
        format!("{name} is used by {used}.")
    }
}

/// The cards that reference `design` and its open pins.
pub fn of_design<R: Reader + ?Sized>(
    r: &R,
    project: ProjectId,
    design: DesignId,
) -> AppResult<DesignUse> {
    let cards = project_tickets(r, project)?
        .into_iter()
        .filter(|t| t.designs.iter().any(|d| d.design == design))
        .map(|t| t.key)
        .collect();
    let pins = r
        .design_pins(project)?
        .iter()
        .filter(|p| p.design == design && p.is_open())
        .count();
    Ok(DesignUse {
        cards,
        pins: u32::try_from(pins).unwrap_or(u32::MAX),
    })
}

/// Takes `design` out of every card and deletes its open pins, with a system comment on each
/// card that used it. Answered pins stay: their answers are already in the cards' prompts.
pub fn release<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    design: DesignId,
    name: &str,
) -> AppResult<DesignUse> {
    let used = of_design(&*uow, project, design)?;
    for key in &used.cards {
        let card = uow
            .ticket_by_key(key)?
            .ok_or_else(|| AppError::not_found(format!("card {key}")))?;
        with_chain(uow, card.chain_id, |ctx| {
            ctx.edit_ticket(card.id, |t| t.designs.retain(|d| d.design != design))
                .map_err(AppError::from)
        })?;
        let view = uow
            .chain_view(card.chain_id)?
            .ok_or_else(|| AppError::not_found("chain"))?;
        let body = CommentBody {
            gist: format!("Design {name} was removed from the library"),
            what: Some(format!(
                "{name} is gone from the project's designs; {key} no longer references it."
            )),
            ..CommentBody::default()
        };
        add_comment(
            uow,
            &view,
            card.id,
            Actor::System,
            CommentKind::Report,
            body,
        )?;
    }
    for pin in uow
        .design_pins(project)?
        .into_iter()
        .filter(|p| p.design == design && p.is_open())
    {
        uow.delete_design_pin(pin.id)?;
        uow.emit(&DomainEvent::DesignPinRemoved { pin })?;
    }
    Ok(used)
}
