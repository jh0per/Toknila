//! The SQLite store as the application's ports.

use serde_json::Value;
use toknila_app::{AppError, AppResult, Reader, UnitOfWork};
use toknila_core::design::pin::DesignPin;
use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, AgentRole, Chain, ChainId, Comment, PinId, Project, ProjectId, Release, ReleaseId, Ship,
    Subtask, Thread, ThreadId, ThreadMessage, ThreadVersion, Ticket, TicketId, TicketKey,
    Timestamp, suggestion::Suggestion,
};

use super::{Repo, StoreError, WriteTx};

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        if let StoreError::Sqlite(rusqlite::Error::SqliteFailure(f, msg)) = &e
            && f.code == rusqlite::ErrorCode::ConstraintViolation
        {
            return AppError::conflict(format!(
                "already exists: {}",
                msg.as_deref().unwrap_or("constraint violation")
            ));
        }
        tracing::error!(error = %e, "store error");
        AppError::internal(e.to_string())
    }
}

/// Implements [`Reader`] for the repository and the write transaction from one method list.
macro_rules! reader {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $out:ty;)*) => {
        impl Reader for Repo<'_> {
            $(fn $name(&self, $($arg: $ty),*) -> AppResult<$out> {
                Ok(Repo::$name(self, $($arg),*)?)
            })*
        }

        impl Reader for WriteTx<'_> {
            $(fn $name(&self, $($arg: $ty),*) -> AppResult<$out> {
                Ok(Repo::$name(&self.repo(), $($arg),*)?)
            })*
        }
    };
}

reader! {
    projects() -> Vec<Project>;
    chain_by_name(project: ProjectId, name: &str) -> Option<Chain>;
    project(id: ProjectId) -> Option<Project>;
    project_by_key(key: &str) -> Option<Project>;
    chain(id: ChainId) -> Option<Chain>;
    chains(project: Option<ProjectId>) -> Vec<Chain>;
    chains_named(name: &str) -> Vec<Chain>;
    ticket(id: TicketId) -> Option<Ticket>;
    ticket_by_key(key: &TicketKey) -> Option<Ticket>;
    tickets(chain: Option<ChainId>) -> Vec<Ticket>;
    release(id: ReleaseId) -> Option<Release>;
    releases_by_version(version: &str) -> Vec<Release>;
    chain_view(chain: ChainId) -> Option<ChainView>;
    subtasks(ticket: TicketId) -> Vec<Subtask>;
    comments(ticket: TicketId) -> Vec<Comment>;
    threads(ticket: TicketId) -> Vec<Thread>;
    thread_messages(thread: ThreadId) -> Vec<ThreadMessage>;
    thread_versions(thread: ThreadId) -> Vec<ThreadVersion>;
    ship(ticket: TicketId) -> Option<Ship>;
    design_pins(project: ProjectId) -> Vec<DesignPin>;
    design_pin(id: PinId) -> Option<DesignPin>;
    suggestion(project: ProjectId) -> Option<Suggestion>;
    last_event_payload(entity: &str, entity_id: &str) -> Option<Value>;
    live_runners() -> Vec<(TicketId, AgentRole)>;
}

impl UnitOfWork for WriteTx<'_> {
    fn now(&self) -> Timestamp {
        WriteTx::now(self)
    }

    fn actor(&self) -> Actor {
        WriteTx::actor(self)
    }

    fn insert_project(&mut self, project: &Project) -> AppResult<()> {
        Ok(self.repo().insert_project(project)?)
    }

    fn update_project(&mut self, project: &Project) -> AppResult<()> {
        Ok(self.repo().update_project(project)?)
    }

    fn delete_project(&mut self, project: ProjectId) -> AppResult<()> {
        Ok(self.repo().delete_project(project)?)
    }

    fn delete_chain(&mut self, chain: ChainId) -> AppResult<()> {
        Ok(self.repo().delete_chain(chain)?)
    }

    fn insert_chain(&mut self, chain: &Chain) -> AppResult<()> {
        Ok(self.repo().insert_chain(chain)?)
    }

    fn take_ticket_number(&mut self, project: ProjectId) -> AppResult<u32> {
        Ok(self.repo().take_ticket_number(project)?)
    }

    fn save_chain_view(&mut self, view: &ChainView) -> AppResult<()> {
        Ok(self.repo().save_chain_view(view)?)
    }

    fn upsert_comment(&mut self, comment: &Comment) -> AppResult<()> {
        Ok(self.repo().upsert_comment(comment)?)
    }

    fn upsert_subtask(&mut self, subtask: &Subtask) -> AppResult<()> {
        Ok(self.repo().upsert_subtask(subtask)?)
    }

    fn upsert_design_pin(&mut self, pin: &DesignPin) -> AppResult<()> {
        Ok(self.repo().upsert_design_pin(pin)?)
    }

    fn delete_design_pin(&mut self, id: PinId) -> AppResult<()> {
        Ok(self.repo().delete_design_pin(id)?)
    }

    fn upsert_thread(&mut self, thread: &Thread) -> AppResult<()> {
        Ok(self.repo().upsert_thread(thread)?)
    }

    fn insert_thread_message(&mut self, message: &ThreadMessage) -> AppResult<()> {
        Ok(self.repo().insert_thread_message(message)?)
    }

    fn insert_thread_version(&mut self, version: &ThreadVersion) -> AppResult<()> {
        Ok(self.repo().insert_thread_version(version)?)
    }

    fn save_ship(&mut self, ship: &Ship) -> AppResult<()> {
        Ok(self.repo().save_ship(ship)?)
    }

    fn save_suggestion(&mut self, project: ProjectId, s: &Suggestion) -> AppResult<()> {
        Ok(self.repo().save_suggestion(project, s)?)
    }

    fn delete_ship(&mut self, ticket: TicketId) -> AppResult<()> {
        Ok(self.repo().delete_ship(ticket)?)
    }

    fn defer(&mut self, effect: toknila_app::external::ExternalEffect) {
        self.deferred.push(effect);
    }

    fn emit(&mut self, event: &DomainEvent) -> AppResult<()> {
        Ok(WriteTx::emit(self, event)?)
    }
}
