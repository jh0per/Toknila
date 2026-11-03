//! API errors: one JSON shape, `{"error": "...", "code": "..."}`, with a fitting status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use toknila_core::graph::GraphError;
use toknila_core::machine::TransitionError;

use crate::store::StoreError;

/// Error body of every failed request.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
    /// Stable machine code: not_found, bad_request, conflict, ambiguous, internal.
    pub code: String,
}

/// An error that maps to an HTTP status.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn not_found(what: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{what} not found"),
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: msg.into(),
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: msg.into(),
        }
    }

    pub fn ambiguous(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "ambiguous",
            message: msg.into(),
        }
    }

    fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
            code: self.code.to_owned(),
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        if let StoreError::Sqlite(rusqlite::Error::SqliteFailure(f, msg)) = &e
            && f.code == rusqlite::ErrorCode::ConstraintViolation
        {
            return ApiError::conflict(format!(
                "already exists: {}",
                msg.as_deref().unwrap_or("constraint violation")
            ));
        }
        tracing::error!(error = %e, "store error");
        ApiError::internal(e.to_string())
    }
}

impl From<TransitionError> for ApiError {
    fn from(e: TransitionError) -> Self {
        match e {
            TransitionError::EmptyText(_) => ApiError::bad_request(e.to_string()),
            _ => ApiError::conflict(e.to_string()),
        }
    }
}

impl From<GraphError> for ApiError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::Transition(t) => t.into(),
            GraphError::UnknownTicket(_) | GraphError::UnknownRelease(_) => ApiError::not_found(e),
            GraphError::NoFixpoint(_) => ApiError::internal(e.to_string()),
            _ => ApiError::conflict(e.to_string()),
        }
    }
}
