//! Ports: what the use cases need from storage. The daemon implements them over SQLite.

use serde_json::Value;
use toknila_core::conversation::Message;
use toknila_core::design::pin::DesignPin;
use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, AgentRole, Chain, ChainId, Comment, PinId, Project, ProjectId, Release, ReleaseId, Ship,
    Subtask, Thread, ThreadId, ThreadMessage, ThreadVersion, Ticket, TicketId, TicketKey,
    Timestamp, suggestion::Suggestion,
};

use crate::AppResult;

/// Read access to the current state.
pub trait Reader {
    /// Every project.
    fn projects(&self) -> AppResult<Vec<Project>>;
    /// A project by id.
    fn project(&self, id: ProjectId) -> AppResult<Option<Project>>;
    /// A project by its upper-case key.
    fn project_by_key(&self, key: &str) -> AppResult<Option<Project>>;
    /// A chain by id.
    fn chain(&self, id: ChainId) -> AppResult<Option<Chain>>;
    /// Every chain, or the chains of one project.
    fn chains(&self, project: Option<ProjectId>) -> AppResult<Vec<Chain>>;
    /// A project's chain by name.
    fn chain_by_name(&self, project: ProjectId, name: &str) -> AppResult<Option<Chain>>;
    /// Chains with exactly this name, across projects.
    fn chains_named(&self, name: &str) -> AppResult<Vec<Chain>>;
    /// A ticket by id.
    fn ticket(&self, id: TicketId) -> AppResult<Option<Ticket>>;
    /// A ticket by key.
    fn ticket_by_key(&self, key: &TicketKey) -> AppResult<Option<Ticket>>;
    /// Every ticket, or the tickets of one chain.
    fn tickets(&self, chain: Option<ChainId>) -> AppResult<Vec<Ticket>>;
    /// A milestone by id.
    fn release(&self, id: ReleaseId) -> AppResult<Option<Release>>;
    /// Milestones with this version, across chains.
    fn releases_by_version(&self, version: &str) -> AppResult<Vec<Release>>;
    /// A chain with its tickets, milestones and edges.
    fn chain_view(&self, chain: ChainId) -> AppResult<Option<ChainView>>;
    /// A ticket's checklist in order.
    fn subtasks(&self, ticket: TicketId) -> AppResult<Vec<Subtask>>;
    /// A ticket's comments, oldest first.
    fn comments(&self, ticket: TicketId) -> AppResult<Vec<Comment>>;
    /// A ticket's threads, oldest first.
    fn threads(&self, ticket: TicketId) -> AppResult<Vec<Thread>>;
    /// A thread's messages, oldest first.
    fn thread_messages(&self, thread: ThreadId) -> AppResult<Vec<ThreadMessage>>;
    /// A thread's artifact versions, by number.
    fn thread_versions(&self, thread: ThreadId) -> AppResult<Vec<ThreadVersion>>;
    /// The project's standing suggestion from its planner, if it has one.
    fn suggestion(&self, project: ProjectId) -> AppResult<Option<Suggestion>>;
    /// The project's conversation with its assistant: the newest `limit` messages that are older
    /// than the message `before` (the newest when unset), oldest first. A card discussion's
    /// messages are among them, read from the card's thread.
    fn conversation_page(
        &self,
        project: ProjectId,
        before: Option<&str>,
        limit: usize,
    ) -> AppResult<Vec<Message>>;
    /// The ship a ticket has now, if any.
    fn ship(&self, ticket: TicketId) -> AppResult<Option<Ship>>;
    /// The assistant's pins on a project's designs, oldest first.
    fn design_pins(&self, project: ProjectId) -> AppResult<Vec<DesignPin>>;
    /// One pin by id.
    fn design_pin(&self, id: PinId) -> AppResult<Option<DesignPin>>;
    /// The runner processes that are alive now, by ticket and role, as the host reported them
    /// when this read began. The parallel limit counts them: a worker kept alive through review
    /// and the reviewer next to it each hold a slot. A host without runners has none.
    fn live_runners(&self) -> AppResult<Vec<(TicketId, AgentRole)>> {
        Ok(Vec::new())
    }
    /// [`Reader::live_runners`] with the time each process started. A host that does not know
    /// the times reports the epoch.
    fn live_runs(&self) -> AppResult<Vec<toknila_core::schedule::LiveRun>> {
        Ok(self
            .live_runners()?
            .into_iter()
            .map(|(t, r)| (t, r, toknila_core::Timestamp(0)))
            .collect())
    }
    /// Payload of the newest journal event about one entity that carries a snapshot under
    /// `field`; events without one are skipped.
    fn last_snapshot_payload(
        &self,
        entity: &str,
        entity_id: &str,
        field: &str,
    ) -> AppResult<Option<Value>>;
}

/// One atomic command: reads, writes and journal entries land together or not at all.
pub trait UnitOfWork: Reader {
    /// Time of this command; every row and event it writes uses it.
    fn now(&self) -> Timestamp;
    /// Who issued this command.
    fn actor(&self) -> Actor;
    /// Inserts a new project.
    fn insert_project(&mut self, project: &Project) -> AppResult<()>;
    /// Saves the changed settings of an existing project.
    fn update_project(&mut self, project: &Project) -> AppResult<()>;
    /// Deletes a project and everything that belongs to it (chains, cards, milestones, edges,
    /// notes, threads, ships, agent sessions) with their journal entries. Token usage records
    /// stay, no longer tied to a card: the tokens were spent.
    fn delete_project(&mut self, project: ProjectId) -> AppResult<()>;
    /// Deletes a chain and everything that belongs to it (cards, milestones, edges, notes,
    /// threads, ships, agent sessions) with their journal entries, as `delete_project` does.
    fn delete_chain(&mut self, chain: ChainId) -> AppResult<()>;
    /// Inserts a new, empty chain.
    fn insert_chain(&mut self, chain: &Chain) -> AppResult<()>;
    /// Takes the next ticket number of a project; never handed out twice.
    fn take_ticket_number(&mut self, project: ProjectId) -> AppResult<u32>;
    /// Saves a chain's tickets, milestones and edges.
    fn save_chain_view(&mut self, view: &ChainView) -> AppResult<()>;
    /// Inserts or replaces a comment.
    fn upsert_comment(&mut self, comment: &Comment) -> AppResult<()>;
    /// Inserts or replaces a subtask.
    fn upsert_subtask(&mut self, subtask: &Subtask) -> AppResult<()>;
    /// Inserts or replaces a thread.
    fn upsert_thread(&mut self, thread: &Thread) -> AppResult<()>;
    /// Stores a new thread message.
    fn insert_thread_message(&mut self, message: &ThreadMessage) -> AppResult<()>;
    /// Stores a new artifact version; versions are never changed after.
    fn insert_thread_version(&mut self, version: &ThreadVersion) -> AppResult<()>;
    /// Stores a new message of the project's conversation; messages are never changed after.
    fn insert_conversation_message(&mut self, project: ProjectId, m: &Message) -> AppResult<()>;
    /// Remembers a ticket's ship, replacing the one it had.
    fn save_ship(&mut self, ship: &Ship) -> AppResult<()>;
    /// Inserts a pin, or sets its answer.
    fn upsert_design_pin(&mut self, pin: &DesignPin) -> AppResult<()>;
    /// Forgets a pin.
    fn delete_design_pin(&mut self, id: PinId) -> AppResult<()>;
    /// Keeps the suggestion as the project's, replacing the one it had.
    fn save_suggestion(&mut self, project: ProjectId, s: &Suggestion) -> AppResult<()>;
    /// Forgets a ticket's ship.
    fn delete_ship(&mut self, ticket: TicketId) -> AppResult<()>;
    /// Queues work on the host for after this command commits (see [`crate::external`]).
    fn defer(&mut self, effect: crate::external::ExternalEffect);
    /// Journals a domain event.
    fn emit(&mut self, event: &DomainEvent) -> AppResult<()>;
}
