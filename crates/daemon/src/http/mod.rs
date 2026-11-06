//! The `/v1` command API and its OpenAPI description, generated from the route handlers.

pub use toknila_api_types as dto;
pub mod error;
pub mod routes;

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::server::AppState;

#[derive(OpenApi)]
#[openapi(
    info(title = "Toknila daemon API", description = "Local API of the Toknila daemon. Every /v1 route needs `Authorization: Bearer <token>` from daemon.json."),
    modifiers(&BearerAuth),
    security(("bearer" = [])),
)]
struct ApiDoc;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
    }
}

/// `/v1` routes (without auth; the server adds it) and the OpenAPI document describing them.
pub fn v1() -> (axum::Router<AppState>, utoipa::openapi::OpenApi) {
    use routes::{
        chains::*, events::*, profiles::*, projects::*, releases::*, runner::*, threads::*,
        tickets::*,
    };
    let (router, mut api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(list_profiles))
        .routes(routes!(list_projects, create_project))
        .routes(routes!(get_project))
        .routes(routes!(create_chain))
        .routes(routes!(import_chain))
        .routes(routes!(export_chain))
        .routes(routes!(list_chains))
        .routes(routes!(get_chain))
        .routes(routes!(add_edge, remove_edge))
        .routes(routes!(create_release))
        .routes(routes!(create_ticket))
        .routes(routes!(propose_wildcard))
        .routes(routes!(list_tickets))
        .routes(routes!(get_ticket, patch_ticket))
        .routes(routes!(ticket_action))
        .routes(routes!(add_comment))
        .routes(routes!(add_subtask))
        .routes(routes!(patch_subtask))
        .routes(routes!(list_threads, open_thread))
        .routes(routes!(post_thread_message))
        .routes(routes!(post_thread_version))
        .routes(routes!(approve_thread))
        .routes(routes!(request_thread_changes))
        .routes(routes!(scrollback))
        .routes(routes!(runner_input))
        .routes(routes!(release_action))
        .routes(routes!(list_events))
        .routes(routes!(agent_event))
        .routes(routes!(release_event))
        .split_for_parts();
    // Routes are nested under /v1; say so in the document.
    api.servers = Some(vec![utoipa::openapi::Server::new("/v1")]);
    (router, api)
}

/// The OpenAPI document as JSON.
pub fn openapi_json() -> String {
    v1().1.to_pretty_json().unwrap_or_default()
}

#[cfg(test)]
mod tests;
