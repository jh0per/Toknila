//! Who may do what: the one table for every caller of the API. A token says who calls
//! ([`Caller`]); the daemon's route lists say which routes it reaches at all; [`may`] says whether
//! it may do an [`Action`] on the [`Target`] the application found. Pure: the application reads
//! the target and asks, and no handler holds a rule of its own.
//!
//! A person may do everything but speak for the assistant. A project's planner plans its own
//! project and starts, accepts and merges nothing (`docs/adr/0005-planner.md`): the routes that
//! would are on no list of its token, and here it edits only what its limits allow. A story's
//! agent acts on its own story only, and what it may report depends on its role.

use crate::conversation::Message;
use crate::planner;
use crate::{Actor, AgentRole, Chain, Project, ProjectId, Ticket, TicketId};

/// Who calls, as its token says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caller {
    /// The person's token.
    Person,
    /// The planner of a project.
    Planner(ProjectId),
    /// A run of a story: its worker, its reviewer, or a release run.
    Card { ticket: TicketId, role: AgentRole },
}

impl Caller {
    /// Who the journal says did it.
    pub fn actor(&self) -> Actor {
        match self {
            Caller::Person => Actor::Human,
            Caller::Planner(_) | Caller::Card { .. } => Actor::Agent,
        }
    }

    /// The project of a planner; nobody else has one.
    pub fn planner_project(&self) -> Option<ProjectId> {
        match self {
            Caller::Planner(p) => Some(*p),
            _ => None,
        }
    }
}

/// What a call does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Read the plan: the overview, an epic or a story in full, a design.
    ReadPlan,
    /// Make an epic. `autostart` as asked; `owner_fields`: it sets a review mode or reviewer.
    NewEpic {
        autostart: Option<bool>,
        owner_fields: bool,
    },
    /// Change an epic. `owner_fields`: its autostart, review mode or reviewer.
    EditEpic { owner_fields: bool },
    /// Add a story to an epic. `owner_fields`: its agent profile, review, reviewer, secrets,
    /// pause, or the stories it goes before.
    AddStory { owner_fields: bool },
    /// Change a story's words, checklist, designs or model. `owner_fields`: anything else.
    EditStory { owner_fields: bool },
    /// Change an epic's graph: add or remove a dependency, move a story in or out of it.
    /// `owner_fields`: a dependency that waits for a release rather than for the story.
    ChangeGraph { owner_fields: bool },
    /// Ask the window to show a story.
    ShowStory,
    /// Open a thread on a story, or write in one.
    Thread,
    /// The assistant's own voice: say something, keep a next step, pin a question on a design.
    Speak,
    /// The planner's turn signal from its hooks.
    ReportTurn,
    /// Report an event of a story's run; `verdict`: a review verdict.
    SendEvent { verdict: bool },
    /// A run's own signals on its story: activity, the permission and pre-tool hooks, claiming
    /// the person's instructions.
    RunSignal,
}

/// What an action is on, as the application found it. A field left empty is not looked at.
#[derive(Debug, Clone, Copy, Default)]
pub struct Target<'a> {
    /// The project the thing belongs to, with its key.
    pub project: Option<(ProjectId, &'a str)>,
    /// The epic the action reads or changes.
    pub epic: Option<&'a Chain>,
    /// The version of the epic's release once it is cut or released.
    pub cut: Option<&'a str>,
    /// The story the action reads or changes.
    pub story: Option<&'a Ticket>,
    /// The conversation from the person's latest message on, oldest first ([`needs_turn`]).
    pub turn: &'a [Message],
}

impl Target<'_> {
    /// How a refusal names the thing.
    fn what(&self) -> String {
        match (self.story, self.epic, self.project) {
            (Some(t), _, _) => format!("story {}", t.key),
            (None, Some(c), _) => format!("epic {}", c.name),
            (None, None, Some((_, key))) => format!("project {key}"),
            (None, None, None) => "it".to_owned(),
        }
    }
}

/// What a planner may not set: the owner's settings of an epic.
const EPIC_SETTINGS: &str = "an epic's autostart, review mode and reviewer";
/// What a planner may not set: the owner's settings of a story.
const STORY_SETTINGS: &str = "a story's agent profile, review mode, reviewer, secrets, pause, \
     the stories it goes before and its place on the canvas";
/// What a planner may not set: a dependency of another kind than a plain one.
const GRAPH_SETTINGS: &str = "a dependency that waits for a release";

/// Why a caller may not do what it asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// The thing belongs to another project than the planner's.
    #[error("{what} belongs to another project; this planner works on {project} only")]
    OtherProject { what: String, project: String },
    /// A change of the graph in a chain that starts its cards by itself could start one.
    #[error(
        "epic {chain} starts its ready stories by itself (autostart), so changing its stories' \
         order or dependencies could start work, and only the owner starts stories. Ask the owner \
         to turn autostart off, or plan in a new epic"
    )]
    AutostartChain { chain: String },
    /// The card's agent has its prompt already, or the card is done.
    #[error(
        "{card} is {status}; the planner edits only stories that have not started. Tell the owner \
         what should change, or open a thread on the story"
    )]
    CardStarted { card: String, status: String },
    /// On the roadmap the planner changes the top level; stories go only to an epic it made in
    /// this turn or one the person named.
    #[error(
        "the person is on the roadmap, where you change epics; stories of epic {chain} are not \
         yours to change now. Add stories only to the epic you made in this turn or one the \
         person named, or ask them to open epic {chain}"
    )]
    RoadmapStory { chain: String },
    /// A cut or released epic is the owner's.
    #[error("release {version} of epic {chain} is cut or released: the epic can no longer change")]
    EpicCut { chain: String, version: String },
    /// A setting only the owner changes.
    #[error("only the owner sets {0}")]
    OwnerOnly(&'static str),
    /// The assistant's own voice and turn are its planner's.
    #[error("these routes are for a project's planner")]
    PlannerOnly,
    /// A planner plans; a story's run signals are its agent's.
    #[error(
        "a planner token may only plan: read the plan, edit epics and stories, write in threads"
    )]
    NotPlanning,
    /// A story's agent reports on its story and does nothing else.
    #[error("an agent token may only send agent events for its own ticket")]
    EventsOnly,
    /// A story's agent acts on its own story only.
    #[error("this agent works on another ticket, not {0}")]
    OtherTicket(String),
    /// A reviewer gives its verdict and reports nothing else.
    #[error("a reviewer agent may only send its review_verdict")]
    VerdictOnly,
    /// Only the reviewer gives a verdict.
    #[error("only the reviewer agent may send a review_verdict")]
    NotTheReviewer,
    /// A planner reports no story's events.
    #[error("a planner may not send story events; it plans through its own tools")]
    PlannerEvents,
    /// A planner reads the repository, so the project needs one.
    #[error("project {0} has no repository for the planner to read")]
    NoRepository(String),
    /// An archived project runs no agents.
    #[error("project {0} is archived")]
    Archived(String),
}

impl Refusal {
    /// Whether the refusal is about the state of the thing (a conflict) rather than who asks.
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Refusal::EpicCut { .. } | Refusal::NoRepository(_) | Refusal::Archived(_)
        )
    }
}

/// Whether [`may`] looks at the conversation turn for this call: the application reads it only
/// then.
pub fn needs_turn(caller: &Caller, action: &Action) -> bool {
    matches!(caller, Caller::Planner(_))
        && matches!(action, Action::AddStory { .. } | Action::EditStory { .. })
}

/// Whether a new epic starts its ready stories by itself: as asked, else as its maker's default
/// (a planner's epic never does; [`may`] refuses a planner that asks for it).
pub fn new_epic_autostart(caller: &Caller, asked: Option<bool>) -> bool {
    asked.unwrap_or(match caller {
        Caller::Planner(_) => planner::NEW_CHAIN_AUTOSTART,
        _ => Chain::DEFAULT_AUTOSTART,
    })
}

/// The model `caller` may name for a story, cleaned: the assistant names an alias or a full
/// model name, a person types any one word. Empty text clears it.
pub fn story_model(caller: &Caller, text: &str) -> Result<Option<String>, String> {
    match caller {
        Caller::Planner(_) => Project::choose_model(text),
        _ => Project::clean_model(text).map_err(str::to_owned),
    }
}

/// Whether a new prompt or acceptance `caller` gives a story the person talks over with the
/// assistant is only a proposal, which lands when the person accepts it. The person's own edit
/// lands at once.
pub fn proposes_edits(caller: &Caller) -> bool {
    matches!(caller, Caller::Planner(_))
}

/// Whether `caller` may do `action` on `target`.
pub fn may(caller: &Caller, action: &Action, target: &Target<'_>) -> Result<(), Refusal> {
    match *caller {
        Caller::Person => match action {
            Action::Speak | Action::ReportTurn => Err(Refusal::PlannerOnly),
            _ => Ok(()),
        },
        Caller::Planner(project) => planner_may(project, action, target),
        Caller::Card { ticket, role } => card_may(ticket, role, action, target),
    }
}

/// A planner plans its own project, within its limits.
fn planner_may(project: ProjectId, action: &Action, t: &Target<'_>) -> Result<(), Refusal> {
    match action {
        Action::SendEvent { .. } => return Err(Refusal::PlannerEvents),
        Action::RunSignal => return Err(Refusal::NotPlanning),
        _ => {}
    }
    if let Some((owner, key)) = t.project {
        planner::ensure_in_project(project, owner, t.what(), key)?;
    }
    match *action {
        Action::NewEpic {
            autostart,
            owner_fields,
        } => {
            if owner_fields || autostart == Some(true) {
                return Err(Refusal::OwnerOnly(EPIC_SETTINGS));
            }
        }
        Action::EditEpic { owner_fields } => {
            if owner_fields {
                return Err(Refusal::OwnerOnly(EPIC_SETTINGS));
            }
            if let (Some(c), Some(version)) = (t.epic, t.cut) {
                return Err(Refusal::EpicCut {
                    chain: c.name.clone(),
                    version: version.to_owned(),
                });
            }
        }
        Action::AddStory { owner_fields } => {
            if owner_fields {
                return Err(Refusal::OwnerOnly(STORY_SETTINGS));
            }
            if let Some(c) = t.epic {
                planner::ensure_graph_change(c)?;
                planner::ensure_story_target(t.turn, c)?;
            }
        }
        Action::EditStory { owner_fields } => {
            if owner_fields {
                return Err(Refusal::OwnerOnly(STORY_SETTINGS));
            }
            if let Some(s) = t.story {
                planner::ensure_card_editable(s)?;
            }
            if let Some(c) = t.epic {
                planner::ensure_story_target(t.turn, c)?;
            }
        }
        Action::ChangeGraph { owner_fields } => {
            if owner_fields {
                return Err(Refusal::OwnerOnly(GRAPH_SETTINGS));
            }
            if let Some(c) = t.epic {
                planner::ensure_graph_change(c)?;
            }
        }
        Action::ReadPlan
        | Action::ShowStory
        | Action::Thread
        | Action::Speak
        | Action::ReportTurn
        | Action::SendEvent { .. }
        | Action::RunSignal => {}
    }
    Ok(())
}

/// A story's run acts on its own story, and reports what its role may.
fn card_may(
    ticket: TicketId,
    role: AgentRole,
    action: &Action,
    t: &Target<'_>,
) -> Result<(), Refusal> {
    if let Some(s) = t.story
        && s.id != ticket
    {
        return Err(Refusal::OtherTicket(s.key.to_string()));
    }
    match *action {
        Action::RunSignal => Ok(()),
        Action::SendEvent { verdict } => match (role, verdict) {
            (AgentRole::Reviewer, true) | (AgentRole::Worker | AgentRole::Release, false) => Ok(()),
            (AgentRole::Reviewer, false) => Err(Refusal::VerdictOnly),
            (AgentRole::Planner, false) => Err(Refusal::PlannerEvents),
            (_, true) => Err(Refusal::NotTheReviewer),
        },
        _ => Err(Refusal::EventsOnly),
    }
}

#[cfg(test)]
mod tests;
