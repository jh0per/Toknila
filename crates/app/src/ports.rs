//! Ports: what the use cases need from storage. The daemon implements them over SQLite.

use serde_json::Value;
use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, Chain, ChainId, Comment, Project, ProjectId, Release, ReleaseId, Ship, Subtask, Thread,
    ThreadId, ThreadMessage, ThreadVersion, Ticket, TicketId, TicketKey, Timestamp,
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
    /// The ship a ticket has now, if any.
    fn ship(&self, ticket: TicketId) -> AppResult<Option<Ship>>;
    /// Payload of the newest journal event about one entity.
    fn last_event_payload(&self, entity: &str, entity_id: &str) -> AppResult<Option<Value>>;
}

/// One atomic command: reads, writes and journal entries land together or not at all.
pub trait UnitOfWork: Reader {
    /// Time of this command; every row and event it writes uses it.
    fn now(&self) -> Timestamp;
    /// Who issued this command.
    fn actor(&self) -> Actor;
    /// Inserts a new project.
    fn insert_project(&mut self, project: &Project) -> AppResult<()>;
    /// Replaces a project's settings (not its key).
    fn save_project(&mut self, project: &Project) -> AppResult<()>;
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
    /// Remembers a ticket's ship, replacing the one it had.
    fn save_ship(&mut self, ship: &Ship) -> AppResult<()>;
    /// Forgets a ticket's ship.
    fn delete_ship(&mut self, ticket: TicketId) -> AppResult<()>;
    /// Queues work on the host for after this command commits (see [`crate::external`]).
    fn defer(&mut self, effect: crate::external::ExternalEffect);
    /// Journals a domain event.
    fn emit(&mut self, event: &DomainEvent) -> AppResult<()>;
}
