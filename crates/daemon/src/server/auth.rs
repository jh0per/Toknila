//! Bearer-token check for every `/v1` route, and who the token says the caller is.
//!
//! Two kinds of token exist. The person's token (`daemon.json`) may call every route. An agent
//! token (one per run, [`crate::runner::AgentTokens`]) may call only the routes in
//! [`AGENT_ROUTES`], for its own ticket; everything else, including every route added later, is
//! refused with `403` until it is listed there on purpose.

use axum::Json;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use toknila_app::{AppError, AppResult, agent};
use toknila_core::agent::AgentEvent;
use toknila_core::{AgentRole, Ticket};

use super::{AppState, now};
use crate::http::error::ErrorBody;
use crate::runner::Grant;

/// The only routes an agent token may call (all `POST`): the agent event contract, the activity
/// signal of hooks and the claim of pending instructions (the Stop hook). The ticket in the path
/// must be the token's own.
pub const AGENT_ROUTES: [&str; 3] = [
    "/v1/tickets/{ticket}/events",
    "/v1/tickets/{ticket}/activity",
    "/v1/tickets/{ticket}/instructions/claim",
];

/// Who is calling, as the token says. Handlers of agent routes check [`Principal::owns`] and, for
/// events, [`Principal::may_send`].
#[derive(Debug, Clone, Copy)]
pub enum Principal {
    /// The person's token.
    Person,
    /// A run's token.
    Agent(Grant),
}

impl Principal {
    /// An agent may act on its own ticket only; a person on any.
    pub fn owns(&self, ticket: &Ticket) -> AppResult<()> {
        match self {
            Principal::Person => Ok(()),
            Principal::Agent(grant) => agent::ensure_granted(grant.ticket, ticket),
        }
    }

    /// What an agent may report depends on its role: a worker reports its work, a reviewer
    /// only gives its verdict on it. A person may send any event (a simulated agent is one).
    pub fn may_send(&self, event: &AgentEvent) -> AppResult<()> {
        let Principal::Agent(grant) = self else {
            return Ok(());
        };
        match (grant.role, event.is_review_verdict()) {
            (AgentRole::Reviewer, true) | (AgentRole::Worker, false) => Ok(()),
            (AgentRole::Reviewer, false) => Err(AppError::forbidden(
                "a reviewer agent may only send its review_verdict",
            )),
            (_, true) => Err(AppError::forbidden(
                "only the reviewer agent may send a review_verdict",
            )),
            (AgentRole::Release, false) => Ok(()),
        }
    }
}

fn refuse(status: StatusCode, error: &str, code: &str) -> Response {
    let body = ErrorBody {
        error: error.to_owned(),
        code: code.to_owned(),
        field: None,
    };
    (status, Json(body)).into_response()
}

/// The token the request carries: `Authorization: Bearer`, or `?token=` (browsers cannot set
/// headers on a WebSocket).
fn presented(req: &Request) -> Option<String> {
    let query = req.uri().query().and_then(|q| {
        q.split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == "token")
            .map(|(_, v)| v.to_owned())
    });
    query.or_else(|| {
        let value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
        value.strip_prefix("Bearer ").map(str::to_owned)
    })
}

/// Lets a request through with a known token, and tells the handler who it is.
pub(super) async fn require_token(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(token) = presented(&req) else {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "missing or wrong bearer token",
            "unauthorized",
        );
    };
    let principal = if constant_time_eq(token.as_bytes(), state.token.as_bytes()) {
        Principal::Person
    } else if let Some(grant) = state.runners.tokens().lookup(&token) {
        let allowed = req.method() == Method::POST
            && req
                .extensions()
                .get::<MatchedPath>()
                .is_some_and(|m| AGENT_ROUTES.contains(&m.as_str()));
        if !allowed {
            return refuse(
                StatusCode::FORBIDDEN,
                "an agent token may only send agent events for its own ticket",
                "forbidden",
            );
        }
        if grant.role == AgentRole::Worker {
            state.runners.tokens().touch(grant.ticket, now(), "api");
        }
        Principal::Agent(grant)
    } else {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "missing or wrong bearer token",
            "unauthorized",
        );
    };
    req.extensions_mut().insert(principal);
    next.run(req).await
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
