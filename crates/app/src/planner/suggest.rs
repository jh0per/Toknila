//! The planner's standing suggestion: the next step it tells the owner, one per project.

use serde::{Deserialize, Serialize};
use toknila_core::ProjectId;
use toknila_core::conversation::{self, About};
use toknila_core::events::DomainEvent;
use toknila_core::suggestion::{self, Suggestion, SuggestionAction};

use super::card;
use crate::{AppError, AppResult, UnitOfWork};

/// What the planner says as its next step.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewSuggestion {
    /// The step in a line or two; with no card and no action it says nothing waits for the owner.
    pub text: String,
    /// The card it is about (key, id or title).
    #[serde(default)]
    pub card: Option<String>,
    /// What the button does: start, answer, accept or release.
    #[serde(default)]
    pub action: Option<SuggestionAction>,
}

/// Keeps `new` as the project's suggestion, replacing the one it had, and journals one
/// `window.suggest` event. Nothing in the plan changes and no action is done.
pub fn suggest<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    new: NewSuggestion,
) -> AppResult<Suggestion> {
    let p = uow
        .project(project)?
        .ok_or_else(|| AppError::not_found(format!("project {project}")))?;
    let ticket = new
        .card
        .as_deref()
        .map(|r| card(&*uow, project, r))
        .transpose()?;
    suggestion::check(&new.text, new.action, ticket.as_ref())
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    let s = Suggestion {
        project: p.key,
        text: new.text.trim().to_owned(),
        card: ticket.as_ref().map(|t| t.key.clone()),
        chain: ticket.as_ref().map(|t| t.chain_id),
        action: new.action,
        at: uow.now(),
    };
    uow.save_suggestion(project, &s)?;
    let about = s.card.clone().map(|key| About::Card { key });
    crate::conversation::line(uow, project, conversation::suggested(&s.text), about)?;
    uow.emit(&DomainEvent::WindowSuggest {
        project,
        suggestion: s.clone(),
    })?;
    Ok(s)
}
