//! Bearer-token check for every `/v1` route.

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::AppState;
use crate::http::error::ErrorBody;

/// Lets a request through only with the daemon's token, in the header or as `?token=`.
pub(super) async fn require_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let expected = format!("Bearer {}", state.token);
    // Browsers cannot set headers on a WebSocket, so `?token=` is accepted too.
    let query_ok = req.uri().query().is_some_and(|q| {
        q.split('&')
            .filter_map(|kv| kv.split_once('='))
            .any(|(k, v)| k == "token" && constant_time_eq(v.as_bytes(), state.token.as_bytes()))
    });
    let ok = query_ok
        || req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| constant_time_eq(v.as_bytes(), expected.as_bytes()));
    if ok {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "missing or wrong bearer token".to_owned(),
                code: "unauthorized".to_owned(),
            }),
        )
            .into_response()
    }
}

pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
