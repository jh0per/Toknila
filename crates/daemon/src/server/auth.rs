//! Bearer-token check for every `/v1` route, and who the token says the caller is.
//!
//! Two kinds of token exist. The person's token (`daemon.json`) may call every route. An agent
//! token (one per run, [`crate::runner::AgentTokens`]) may call only the routes in
//! [`AGENT_ROUTES`], for its own ticket; everything else, including every route added later, is
//! refused with `403` until it is listed there on purpose. A planner's token (its role says so)
//! reaches [`PLANNER_ROUTES`] and nothing else, so it cannot start, pause, resume, cancel,
//! accept, return, reopen or merge anything: those routes are not on the list. On a listed route,
//! what the caller may do to what is core's `access::may` ([`Principal::caller`]), asked by the
//! use case; no handler holds a rule of its own.

use axum::Json;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use toknila_app::AppResult;
use toknila_app::access::{self, Action, Caller};
use toknila_core::access::{self as rule, Refusal, Target};
use toknila_core::agent::AgentEvent;
use toknila_core::{AgentRole, ProjectId, RunId, Ticket, planner};

use super::{AppState, now};
use crate::http::error::ErrorBody;
use crate::runner::Grant;

/// The only routes an agent token may call (all `POST`): the agent event contract, the activity
/// signal of hooks, the claim of pending instructions (the Stop hook) and the plan quota a status
/// line saw (account-wide, so it names no ticket). The ticket in a path must be the token's own.
pub const AGENT_ROUTES: [&str; 6] = [
    "/v1/tickets/{ticket}/events",
    "/v1/tickets/{ticket}/activity",
    "/v1/tickets/{ticket}/permission",
    "/v1/tickets/{ticket}/pre-tool",
    "/v1/tickets/{ticket}/instructions/claim",
    "/v1/usage/quota",
];

/// The only routes a planner's token may call, as `(method, path)`: the person's routes of the
/// actions a planner has (TOKNILA-149), its own voice (`say`) and turn (`activity`), and, for one
/// release, its old `/planner/...` routes as deprecated aliases. Every route here runs a use case
/// that asks core's `access::may` as the caller the token names, so a planner reaches only its
/// own project and only what its limits allow; nothing here starts, accepts or merges.
pub const PLANNER_ROUTES: [(&str, &str); 40] = [
    ("GET", "/v1/projects/{project}/plan"),
    ("GET", "/v1/projects/{project}/plan/overview"),
    ("GET", "/v1/projects/{project}/designs/{design}"),
    ("GET", "/v1/chains/{chain}"),
    ("GET", "/v1/chains/{chain}/plan"),
    ("GET", "/v1/tickets/{ticket}/plan"),
    ("POST", "/v1/projects/{project}/design-pins"),
    ("POST", "/v1/projects/{project}/chains"),
    ("PATCH", "/v1/chains/{chain}"),
    ("POST", "/v1/chains/{chain}/tickets"),
    ("PATCH", "/v1/tickets/{ticket}"),
    ("POST", "/v1/tickets/{ticket}/show"),
    ("PUT", "/v1/projects/{project}/suggestion"),
    ("POST", "/v1/chains/{chain}/edges"),
    ("DELETE", "/v1/chains/{chain}/edges"),
    ("POST", "/v1/chains/{chain}/tickets/{ticket}/move"),
    ("POST", "/v1/tickets/{ticket}/move-to-chain"),
    ("POST", "/v1/tickets/{ticket}/threads"),
    ("POST", "/v1/tickets/{ticket}/threads/{thread}/messages"),
    ("POST", "/v1/tickets/{ticket}/threads/{thread}/versions"),
    ("POST", "/v1/planner/activity"),
    ("POST", "/v1/planner/say"),
    ("GET", "/v1/planner/plan"),
    ("GET", "/v1/planner/overview"),
    ("GET", "/v1/planner/chains/{chain}"),
    ("GET", "/v1/planner/cards/{card}"),
    ("GET", "/v1/planner/designs/{design}"),
    ("POST", "/v1/planner/pins"),
    ("POST", "/v1/planner/chains"),
    ("PATCH", "/v1/planner/chains/{chain}"),
    ("POST", "/v1/planner/chains/{chain}/cards"),
    ("PATCH", "/v1/planner/cards/{card}"),
    ("POST", "/v1/planner/cards/{card}/show"),
    ("POST", "/v1/planner/suggestion"),
    ("POST", "/v1/planner/chains/{chain}/edges"),
    ("DELETE", "/v1/planner/chains/{chain}/edges"),
    ("POST", "/v1/planner/chains/{chain}/cards/{card}/move"),
    ("POST", "/v1/planner/cards/{card}/threads"),
    ("POST", "/v1/planner/cards/{card}/threads/{thread}/messages"),
    ("POST", "/v1/planner/cards/{card}/threads/{thread}/versions"),
];

/// Who is calling, as the token says. What it may do is core's `access::may`, asked through
/// [`Principal::caller`] by the use cases and, for a story's run, by [`Principal::owns`] and
/// [`Principal::may_send`].
#[derive(Debug, Clone, Copy)]
pub enum Principal {
    /// The person's token.
    Person,
    /// A run's token.
    Agent(Grant),
}

impl Principal {
    /// The caller the access rule knows: the person, a project's planner, or a story's run.
    pub fn caller(&self) -> Caller {
        match self {
            Principal::Person => Caller::Person,
            Principal::Agent(grant) if grant.role == AgentRole::Planner => {
                Caller::Planner(planner::project_of(grant.ticket))
            }
            Principal::Agent(grant) => Caller::Card {
                ticket: grant.ticket,
                role: grant.role,
            },
        }
    }

    /// An agent may act on its own ticket only; a person on any.
    pub fn owns(&self, ticket: &Ticket) -> AppResult<()> {
        access::on_run_story(&self.caller(), &Action::RunSignal, ticket)
    }

    /// The project a planner's token works on. Only a planner has one: a person uses the project
    /// routes, and a card's agent has its ticket.
    pub fn planner_project(&self) -> AppResult<ProjectId> {
        Ok(self
            .caller()
            .planner_project()
            .ok_or(Refusal::PlannerOnly)?)
    }

    /// The project and the run of a planner's token; anyone else is refused as in
    /// [`Principal::planner_project`].
    pub fn planner_run(&self) -> AppResult<(ProjectId, RunId)> {
        let project = self.planner_project()?;
        match self {
            Principal::Agent(grant) => Ok((project, grant.run)),
            Principal::Person => Err(Refusal::PlannerOnly.into()),
        }
    }

    /// What an agent may report depends on its role: a worker reports its work, a reviewer
    /// only gives its verdict on it. A person may send any event (a simulated agent is one).
    pub fn may_send(&self, event: &AgentEvent) -> AppResult<()> {
        let report = Action::SendEvent {
            verdict: event.is_review_verdict(),
        };
        Ok(rule::may(&self.caller(), &report, &Target::default())?)
    }
}

fn refuse(status: StatusCode, error: &str, code: &str) -> Response {
    let body = ErrorBody {
        error: error.to_owned(),
        code: code.to_owned(),
        field: None,
        used_by: None,
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
        let matched = req
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str);
        let allowed = match grant.role {
            AgentRole::Planner => matched.is_some_and(|path| {
                PLANNER_ROUTES
                    .iter()
                    .any(|(method, route)| *method == req.method().as_str() && *route == path)
            }),
            _ => req.method() == Method::POST && matched.is_some_and(|p| AGENT_ROUTES.contains(&p)),
        };
        if !allowed {
            let what = if grant.role == AgentRole::Planner {
                "a planner token may only plan: read the plan, edit epics and stories, write in threads"
            } else {
                "an agent token may only send agent events for its own ticket"
            };
            return refuse(StatusCode::FORBIDDEN, what, "forbidden");
        }
        // Any call of a run's token is a sign of life; the startup rule counts it for either role.
        state
            .runners
            .tokens()
            .mark_active(grant.ticket, grant.role, now());
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
