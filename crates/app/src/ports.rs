//! Ports: what the use cases need from storage. The daemon implements them over SQLite.

use serde_json::Value;
use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, Chain, ChainId, Comment, Project, ProjectId, Release, ReleaseId, Subtask, Ticket,
    TicketId, TicketKey, Timestamp,
};

use crate::AppResult;

/// Read access to the current state.
pub trait Reader {
    /// A project by id.
    fn project(&self, id: ProjectId) -> AppResult<Option<Project>>;
    /// A project by its upper-case key.
    fn project_by_key(&self, key: &str) -> AppResult<Option<Project>>;
    /// A chain by id.
    fn chain(&self, id: ChainId) -> AppResult<Option<Chain>>;
    /// Every chain, or the chains of one project.
    fn chains(&self, project: Option<ProjectId>) -> AppResult<Vec<Chain>>;
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
    /// Payload of the newest journal event about one entity.
    fn last_event_payload(&self, entity: &str, entity_id: &str) -> AppResult<Option<Value>>;
}

/// One atomic command: reads, writes and journal entries land together or not at all.
pub trait UnitOfWork: Reader {
    /// Time of this command; every row and event it writes uses it.
    fn now(&self) -> Timestamp;
    /// Who issued this command.
    fn actor(&self) -> Actor;
    /// Saves a chain's tickets, milestones and edges.
    fn save_chain_view(&mut self, view: &ChainView) -> AppResult<()>;
    /// Inserts or replaces a comment.
    fn upsert_comment(&mut self, comment: &Comment) -> AppResult<()>;
    /// Inserts or replaces a subtask.
    fn upsert_subtask(&mut self, subtask: &Subtask) -> AppResult<()>;
    /// Journals a domain event.
    fn emit(&mut self, event: &DomainEvent) -> AppResult<()>;
}
