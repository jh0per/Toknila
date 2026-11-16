//! What a caller changes on an existing story: `PATCH /v1/tickets/{ticket}`.

use serde::{Deserialize, Serialize};

use crate::design::{DesignPick, DesignRef, check_refs};

use super::{Pos, ReviewMode, Size, Ticket};

/// Fields a person may change on an existing ticket; `None` keeps the current value. Status is
/// not among them: it moves only through the state machine. Checklist items to rename and to
/// append ride along in the same command; [`TicketPatch::apply`] leaves them to it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema), schema(as = PatchTicket))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketPatch {
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub reviewer: Option<String>,
    /// The card's own model; an empty text clears it (the project's applies again).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub needs_secrets: Option<bool>,
    pub design_first: Option<bool>,
    /// The card's designs by id or file name, replacing the whole list (an empty list clears it);
    /// left out to keep them. The daemon turns a file name into the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designs: Option<Vec<DesignPick>>,
    /// Where the card sits on the canvas: a place, `null` to hand it back to the automatic
    /// layout, left out to keep it.
    #[serde(
        default,
        deserialize_with = "some_or_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Pos>, nullable))]
    pub pos: Option<Option<Pos>>,
    /// The card's own size: a size, `null` to let it fit its text again, left out to keep it.
    #[serde(
        default,
        deserialize_with = "some_or_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Size>, nullable))]
    pub size: Option<Option<Size>>,
    /// Checklist items to rename.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edit_subtasks: Vec<SubtaskEdit>,
    /// Checklist items to append.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_subtasks: Vec<String>,
}

/// A rename of a checklist item.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SubtaskEdit {
    /// 1-based position.
    pub position: u32,
    pub text: String,
}

/// Tells a field that is `null` from one that is left out: `Some(None)` against `None`.
fn some_or_null<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

impl TicketPatch {
    /// Whether it sets what only the owner sets on a story: its agent profile, review mode,
    /// reviewer, secrets, or its place and size on the canvas (`crate::access` decides who may).
    pub fn sets_owner_fields(&self) -> bool {
        self.profile.is_some()
            || self.review_mode.is_some()
            || self.reviewer.is_some()
            || self.needs_secrets.is_some()
            || self.pos.is_some()
            || self.size.is_some()
    }

    /// Whether the patch only moves or resizes the card on the canvas (allowed on a finished card
    /// too).
    pub fn only_moves(&self) -> bool {
        (self.pos.is_some() || self.size.is_some())
            && Self {
                pos: None,
                size: None,
                ..self.clone()
            } == Self::default()
    }

    /// The designs it names as references: every one by id, each region inside the image and
    /// none twice. `None` when it leaves the designs as they are.
    pub fn design_refs(&self) -> Result<Option<Vec<DesignRef>>, String> {
        let Some(picks) = &self.designs else {
            return Ok(None);
        };
        let refs = picks
            .iter()
            .map(DesignPick::to_ref)
            .collect::<Result<Vec<_>, _>>()?;
        check_refs(&refs)?;
        Ok(Some(refs))
    }

    /// Writes the given fields into `t`; the designs were checked with
    /// [`TicketPatch::design_refs`].
    pub fn apply(self, t: &mut Ticket) {
        if let Ok(Some(designs)) = self.design_refs() {
            t.designs = designs;
        }
        if let Some(v) = self.title {
            t.title = v;
        }
        if let Some(v) = self.prompt {
            t.prompt = v;
        }
        if let Some(v) = self.acceptance {
            t.acceptance = v;
        }
        if let Some(v) = self.profile {
            t.profile = Some(v);
        }
        if let Some(v) = self.review_mode {
            t.review_mode = Some(v);
        }
        if let Some(v) = self.reviewer {
            t.reviewer = Some(v).filter(|v| !v.trim().is_empty());
        }
        if let Some(v) = self.model {
            t.model = Some(v.trim().to_string()).filter(|v| !v.is_empty());
        }
        if let Some(v) = self.needs_secrets {
            t.needs_secrets = v;
        }
        if let Some(v) = self.design_first {
            t.design_first = v;
        }
        if let Some(v) = self.pos {
            t.pos = v;
        }
        if let Some(v) = self.size {
            t.size = v;
        }
    }
}
