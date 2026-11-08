//! API errors: one JSON shape, `{"error": "...", "code": "..."}`, with a fitting status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use toknila_app::{AppError, ErrorKind};
use toknila_core::graph::GraphError;
use toknila_core::machine::{TransitionError, UnknownAction};
use toknila_core::repo::RepoProblem;

pub use toknila_api_types::ErrorBody;

use crate::store::StoreError;

/// An application error on its way to an HTTP response.
#[derive(Debug, Clone)]
pub struct ApiError(pub AppError);

impl ApiError {
    /// The HTTP status for this error's kind.
    pub fn status(&self) -> StatusCode {
        match self.0.kind {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Conflict | ErrorKind::Ambiguous => StatusCode::CONFLICT,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
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
            field: None,
        };
        (status, Json(body)).into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

/// A blocking task that panicked or was cancelled.
impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self(AppError::internal(e.to_string()))
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

impl From<UnknownAction> for ApiError {
    fn from(e: UnknownAction) -> Self {
        Self(e.into())
    }
}

impl From<GraphError> for ApiError {
    fn from(e: GraphError) -> Self {
        Self(e.into())
    }
}

/// A refusal that belongs to one input field, for the forms of the window: the folder checks of a
/// project and the profile of its settings. Anything else that goes wrong is an [`ApiError`].
#[derive(Debug)]
pub enum FormError {
    /// `field` is the input, `code` the stable reason.
    Field {
        field: String,
        code: String,
        message: String,
    },
    Api(ApiError),
}

impl FormError {
    /// A refusal of one field.
    pub fn field(field: &str, code: &str, message: impl Into<String>) -> Self {
        Self::Field {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl From<RepoProblem> for FormError {
    fn from(p: RepoProblem) -> Self {
        // The wire names are the serde names of the core enums.
        let name = |v: serde_json::Value| v.as_str().unwrap_or_default().to_owned();
        Self::Field {
            field: name(serde_json::json!(p.field)),
            code: name(serde_json::json!(p.code)),
            message: p.message,
        }
    }
}

impl<E: Into<ApiError>> From<E> for FormError {
    fn from(e: E) -> Self {
        Self::Api(e.into())
    }
}

impl IntoResponse for FormError {
    fn into_response(self) -> Response {
        match self {
            Self::Api(e) => e.into_response(),
            Self::Field {
                field,
                code,
                message,
            } => {
                let body = ErrorBody {
                    code,
                    error: message,
                    field: Some(field),
                };
                (StatusCode::BAD_REQUEST, Json(body)).into_response()
            }
        }
    }
}
