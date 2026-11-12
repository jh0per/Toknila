//! What the planner reads first: the plan of its project in short records, so that it fits in
//! one answer at any size. A chain or a card is then opened in full (`plan::chain_in_full`,
//! `plan::card_in_full`).

use serde::{Deserialize, Serialize};
use toknila_core::design::pin::DesignPin;
use toknila_core::graph::{ChainView, PhaseState};
use toknila_core::thread::awaiting_person;
use toknila_core::{ChainId, DesignId, Project, ProjectId, TicketKey, TicketStatus, Timestamp};

use super::plan::ordered;
use crate::{AppError, AppResult, Reader};

/// One card of an unfinished chain as one short record.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewCard {
    pub key: TicketKey,
    pub title: String,
    pub status: TicketStatus,
    /// What it waits for: card keys and `release <version>` labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub design_first: bool,
    /// The card's own model; absent when the project's runs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Names of the designs the card is about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub designs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subtasks: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub threads: u32,
    /// The card waits for the owner: it is proposed, asks a question, is in review, or has a
    /// thread version the owner has not answered.
    #[serde(default, skip_serializing_if = "is_false")]
    pub waits_for_owner: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Where a chain stands.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewSummary {
    pub phase: PhaseState,
    pub done: u32,
    pub total: u32,
}

/// A chain: its facts, and its cards unless it is finished.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewChain {
    pub id: ChainId,
    pub name: String,
    /// What the owner reads for the epic: its title, or the name made readable.
    #[serde(default)]
    pub title: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub autostart: bool,
    pub summary: OverviewSummary,
    /// Empty for a finished chain: open it with `chain` to read its cards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<OverviewCard>,
}

/// The project, every chain in short, and the pins with their answers.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanOverview {
    pub project: Project,
    pub chains: Vec<OverviewChain>,
    #[serde(default)]
    pub pins: Vec<DesignPin>,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn cards<R: Reader + ?Sized>(
    r: &R,
    view: &ChainView,
    names: &[(DesignId, String)],
) -> AppResult<Vec<OverviewCard>> {
    let mut out = Vec::new();
    for (t, after) in ordered(view) {
        let threads = r.threads(t.id)?;
        let mut owner = matches!(
            t.status,
            TicketStatus::Proposed | TicketStatus::Waiting | TicketStatus::Review
        );
        for th in &threads {
            let versions = r.thread_versions(th.id)?;
            let messages = r.thread_messages(th.id)?;
            owner |= awaiting_person(th, &versions, &messages).is_some();
        }
        let designs = t
            .designs
            .iter()
            .map(|d| {
                names
                    .iter()
                    .find(|(id, _)| *id == d.design)
                    .map_or_else(|| d.design.to_string(), |(_, n)| n.clone())
            })
            .collect();
        out.push(OverviewCard {
            subtasks: count(r.subtasks(t.id)?.len()),
            threads: count(threads.len()),
            key: t.key,
            title: t.title,
            status: t.status,
            after,
            design_first: t.design_first,
            model: t.model,
            designs,
            waits_for_owner: owner,
        });
    }
    Ok(out)
}

/// The overview of `project` at `at`; `names` are the project's designs by id.
pub fn overview<R: Reader + ?Sized>(
    r: &R,
    project: ProjectId,
    at: Timestamp,
    names: &[(DesignId, String)],
) -> AppResult<PlanOverview> {
    let project = r
        .project(project)?
        .ok_or_else(|| AppError::not_found("project"))?;
    let mut chains = Vec::new();
    for chain in r.chains(Some(project.id))? {
        let Some(view) = r.chain_view(chain.id)? else {
            continue;
        };
        let s = view.summary(at);
        let finished = s.phase == PhaseState::Finished;
        chains.push(OverviewChain {
            id: chain.id,
            title: chain.display_title(),
            name: chain.name,
            goal: chain.goal,
            version: chain.version,
            autostart: chain.autostart,
            summary: OverviewSummary {
                phase: s.phase,
                done: count(s.done),
                total: count(s.total),
            },
            cards: if finished {
                Vec::new()
            } else {
                cards(r, &view, names)?
            },
        });
    }
    let pins = r.design_pins(project.id)?;
    Ok(PlanOverview {
        project,
        chains,
        pins,
    })
}
