//! Application errors: a kind that adapters map to their protocol, and a message for humans.

use toknila_core::graph::GraphError;
use toknila_core::inbox::leftovers::LeftoverError;
use toknila_core::machine::{TransitionError, UnknownAction};
use toknila_core::thread::ThreadError;

/// What went wrong, independent of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The referenced thing does not exist.
    NotFound,
    /// The input is malformed or incomplete.
    BadRequest,
    /// The request is valid but the current state forbids it.
    Conflict,
    /// A human reference matches more than one thing.
    Ambiguous,
    /// The caller is known but may not do this (an agent acting on a ticket that is not its own).
    Forbidden,
    /// A bug or a storage failure.
    Internal,
}

impl ErrorKind {
    /// Stable machine code used on the wire.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::NotFound => "not_found",
            ErrorKind::BadRequest => "bad_request",
            ErrorKind::Conflict => "conflict",
            ErrorKind::Ambiguous => "ambiguous",
            ErrorKind::Forbidden => "forbidden",
            ErrorKind::Internal => "internal",
        }
    }
}

/// A failed use case.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

/// Result of a use case.
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// An error of `kind` with `message`.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// `"{what} not found"`.
    pub fn not_found(what: impl std::fmt::Display) -> Self {
        Self::new(ErrorKind::NotFound, format!("{what} not found"))
    }

    /// Malformed or incomplete input.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadRequest, message)
    }

    /// The current state forbids the request.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, message)
    }

    /// A reference matches more than one thing.
    pub fn ambiguous(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Ambiguous, message)
    }

    /// The caller may not do this.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, message)
    }

    /// A bug or a storage failure.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }
}

impl From<TransitionError> for AppError {
    fn from(e: TransitionError) -> Self {
        match e {
            TransitionError::EmptyText(_) => AppError::bad_request(e.to_string()),
            _ => AppError::conflict(e.to_string()),
        }
    }
}

impl From<UnknownAction> for AppError {
    fn from(e: UnknownAction) -> Self {
        AppError::not_found(e.to_string())
    }
}

impl From<LeftoverError> for AppError {
    fn from(e: LeftoverError) -> Self {
        AppError::conflict(e.to_string())
    }
}

impl From<ThreadError> for AppError {
    fn from(e: ThreadError) -> Self {
        match e {
            ThreadError::EmptyTitle
            | ThreadError::EmptyText(_)
            | ThreadError::EmptyArtifact
            | ThreadError::ArtifactTooLarge { .. } => AppError::bad_request(e.to_string()),
            _ => AppError::conflict(e.to_string()),
        }
    }
}

impl From<GraphError> for AppError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::Transition(t) => t.into(),
            GraphError::UnknownTicket(_) | GraphError::UnknownRelease(_) => AppError::not_found(e),
            GraphError::NoFixpoint(_) => AppError::internal(e.to_string()),
            _ => AppError::conflict(e.to_string()),
        }
    }
}
