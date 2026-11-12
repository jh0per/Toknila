//! The assistant's pins on designs: it asks a question about a region of a mockup, the person
//! answers in place, and an answered pin goes into its card's prompt (`## From the design`).

use serde::{Deserialize, Serialize};
use toknila_core::design::Region;
use toknila_core::design::pin::{self, DesignPin};
use toknila_core::events::DomainEvent;
use toknila_core::{
    CommentBody, CommentKind, DesignId, PinId, ProjectId, TicketPatch, TicketStatus,
};

use crate::{AppError, AppResult, Reader, UnitOfWork, planner, tickets};

/// What the assistant gives for a pin. The host has found the design in the project's library
/// and gives its id and name.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewPin {
    pub design: DesignId,
    pub design_name: String,
    /// In whole percent of the image.
    pub region: Region,
    /// The question.
    pub text: String,
    /// The card it is about (id, key or title).
    #[serde(default)]
    pub card: Option<String>,
}

/// What a person answers.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinAnswer {
    pub text: String,
}

/// The pins of a project, oldest first.
pub fn list<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<Vec<DesignPin>> {
    r.design_pins(project)
}

/// Pins a question on a design of the planner's project.
pub fn ask<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    input: NewPin,
) -> AppResult<DesignPin> {
    input.region.check().map_err(AppError::bad_request)?;
    let text = pin::clean_text(&input.text, "question").map_err(AppError::bad_request)?;
    let card = match input
        .card
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        Some(reference) => Some(planner::card(&*uow, project, reference)?.key),
        None => None,
    };
    let number = uow
        .design_pins(project)?
        .iter()
        .filter(|p| p.design == input.design)
        .map(|p| p.number)
        .max()
        .unwrap_or(0)
        + 1;
    let pin = DesignPin {
        id: PinId::new(),
        project,
        design: input.design,
        design_name: input.design_name,
        number,
        region: input.region,
        text,
        card,
        answer: None,
        created_at: uow.now(),
        answered_at: None,
    };
    uow.upsert_design_pin(&pin)?;
    uow.emit(&DomainEvent::DesignPinAsked { pin: pin.clone() })?;
    Ok(pin)
}

/// A person answers a pin, or changes the answer. The card's prompt gets the answered pins of
/// the card under its `From the design` section, unless the card is finished.
pub fn answer<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    id: PinId,
    text: &str,
) -> AppResult<DesignPin> {
    let text = pin::clean_text(text, "answer").map_err(AppError::bad_request)?;
    let mut pin = uow
        .design_pin(id)?
        .filter(|p| p.project == project)
        .ok_or_else(|| AppError::not_found(format!("pin {id}")))?;
    pin.answer = Some(text);
    pin.answered_at = Some(uow.now());
    uow.upsert_design_pin(&pin)?;
    uow.emit(&DomainEvent::DesignPinAnswered { pin: pin.clone() })?;
    if let Some(key) = &pin.card
        && let Some(card) = uow.ticket_by_key(key)?
        && card.status == TicketStatus::Waiting
        && card.agent_paused.is_some()
    {
        // The card's agent has no process: the answer is its first input when it comes back,
        // as a comment is.
        let gist = format!(
            "Design pin {} on {}: {}",
            pin.number,
            pin.design_name,
            pin.answer.as_deref().unwrap_or_default()
        );
        let body = CommentBody {
            gist,
            ..CommentBody::default()
        };
        tickets::comment(uow, &card, CommentKind::Instruction, body)?;
    } else if let Some(key) = &pin.card
        && let Some(card) = uow.ticket_by_key(key)?
        && card.status.can_edit()
    {
        let all = uow.design_pins(project)?;
        let of_card: Vec<&DesignPin> = all
            .iter()
            .filter(|p| p.card.as_ref() == Some(key))
            .collect();
        let prompt = pin::with_section(&card.prompt, &of_card);
        if prompt != card.prompt {
            let patch = TicketPatch {
                prompt: Some(prompt),
                ..TicketPatch::default()
            };
            tickets::edit(uow, &card, patch)?;
        }
    }
    Ok(pin)
}
