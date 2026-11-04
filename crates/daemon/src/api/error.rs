//! API errors: one JSON shape, `{"error": "...", "code": "..."}`, with a fitting status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use toknila_app::{AppError, ErrorKind};
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

/// An application error on its way to an HTTP response.
#[derive(Debug, Clone)]
pub struct ApiError(pub AppError);

impl ApiError {
    pub fn not_found(what: impl std::fmt::Display) -> Self {
        Self(AppError::not_found(what))
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self(AppError::bad_request(msg))
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self(AppError::conflict(msg))
    }

    /// The HTTP status for this error's kind.
    pub fn status(&self) -> StatusCode {
        match self.0.kind {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Conflict | ErrorKind::Ambiguous => StatusCode::CONFLICT,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = ErrorBody {
            code: self.0.kind.code().to_owned(),
            error: self.0.message,
        };
        (status, Json(body)).into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        Self(e.into())
    }
}

impl From<TransitionError> for ApiError {
    fn from(e: TransitionError) -> Self {
        Self(e.into())
    }
}

impl From<GraphError> for ApiError {
    fn from(e: GraphError) -> Self {
        Self(e.into())
    }
}
