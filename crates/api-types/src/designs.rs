//! A project's designs: images, pdfs and Figma frame links (`/v1/projects/{project}/designs`).

use serde::{Deserialize, Serialize};
use toknila_core::design::{DesignKind, FigmaRef};
use toknila_core::{DesignId, Timestamp};

/// One design of a project.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Design {
    pub id: DesignId,
    pub name: String,
    pub kind: DesignKind,
    /// Media type of the kept file; absent for a Figma link whose frame was not fetched.
    pub mime: Option<String>,
    /// Bytes of the kept file; zero without one.
    pub size: u64,
    pub created_at: Timestamp,
    /// The frame a `figma` design points at.
    pub figma: Option<FigmaRef>,
    /// Whether a file is kept, served at `.../designs/{id}/file`.
    pub has_file: bool,
    /// Whether the file can be shown as a picture (everything kept but a pdf).
    pub thumb: bool,
    /// What the person can do about a link without an image: export the frame, or why the fetch
    /// failed. Never holds the token.
    pub hint: Option<String>,
}

/// A design as the planner sees it: its metadata and where its file is on this machine.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlannerDesign {
    pub id: DesignId,
    pub name: String,
    pub kind: DesignKind,
    /// Media type of the kept file.
    pub mime: Option<String>,
    /// Bytes of the kept file; zero without one.
    pub size: u64,
    /// Absolute path of the kept file, which the planner may `Read`; absent when no file is kept.
    pub path: Option<String>,
    /// Why a link has no file yet, when known.
    pub hint: Option<String>,
}

/// Answer of `GET /v1/planner/plan`: the plan and the project's designs.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlannerPlan {
    #[serde(flatten)]
    pub plan: toknila_app::planner::PlanView,
    /// Oldest first.
    pub designs: Vec<PlannerDesign>,
}

/// Answer of `GET /v1/projects/{project}/designs`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DesignList {
    /// Oldest first.
    pub designs: Vec<Design>,
    /// Whether the project has a Figma token; the token itself is never sent.
    pub figma_token: bool,
}

/// Body of `POST /v1/projects/{project}/designs/figma`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewFigmaDesign {
    /// The link of a frame (Copy link to selection).
    pub url: String,
    /// A name; the link's file key when empty.
    #[serde(default)]
    pub name: Option<String>,
}

/// Body of `PATCH /v1/projects/{project}/designs/{design}`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RenameDesign {
    pub name: String,
}

/// Body of `PUT /v1/projects/{project}/figma-token`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize)]
pub struct FigmaToken {
    pub token: String,
}

impl std::fmt::Debug for FigmaToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FigmaToken(..)")
    }
}

/// Body of `POST /v1/planner/pins`: a question the planner pins on a region of a design.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewDesignPin {
    /// The design: its name or id.
    pub design: String,
    /// In whole percent of the image.
    pub region: toknila_core::design::Region,
    /// The question, at most 500 characters.
    pub text: String,
    /// The card it is about (id, key or title).
    #[serde(default)]
    pub card: Option<String>,
}
