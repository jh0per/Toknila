//! The JSON body of a request: axum's `Json`, except that a body the daemon cannot read (an
//! unknown field, a wrong enum value, a missing field) is a 400 in the API's error shape that
//! carries serde's message, which names the offender and lists what is accepted.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, OptionalFromRequest, Request};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::error::ErrorBody;

/// A JSON request body (or response); see the module doc.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// A body that was refused, as the API's error shape.
#[derive(Debug)]
pub struct BodyRejection(JsonRejection);

impl IntoResponse for BodyRejection {
    fn into_response(self) -> Response {
        let status = match self.0 {
            // Valid JSON the types do not accept: a bad request, not "unprocessable".
            JsonRejection::JsonDataError(_) => StatusCode::BAD_REQUEST,
            ref other => other.status(),
        };
        let text = self.0.body_text();
        let message = text
            .strip_prefix("Failed to deserialize the JSON body into the target type: ")
            .or_else(|| text.strip_prefix("Failed to parse the request body as JSON: "))
            .unwrap_or(&text)
            .to_owned();
        let body = ErrorBody {
            code: "bad_request".to_owned(),
            error: message,
            field: None,
        };
        (status, axum::Json(body)).into_response()
    }
}

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = BodyRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        <axum::Json<T> as FromRequest<S>>::from_request(req, state)
            .await
            .map(|axum::Json(v)| Self(v))
            .map_err(BodyRejection)
    }
}

impl<T, S> OptionalFromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = BodyRejection;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        // No body at all is "no input"; a body that is there must be readable.
        if !req.headers().contains_key(header::CONTENT_TYPE) {
            return Ok(None);
        }
        <Self as FromRequest<S>>::from_request(req, state)
            .await
            .map(Some)
    }
}
