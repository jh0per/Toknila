//! The `/v1` command API and its OpenAPI description, generated from the route handlers.

pub use toknila_api_types as dto;
pub mod body;
pub mod error;
pub mod routes;
mod strict;

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::server::AppState;

#[derive(OpenApi)]
#[openapi(
    info(title = "Toknila daemon API", description = "Local API of the Toknila daemon. Every /v1 route needs `Authorization: Bearer <token>` from daemon.json."),
    components(schemas(dto::SizeLimits)),
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
        agents::*, chains::*, conversation::*, designs::*, events::*, inbox::*, pins::*,
        planner::*, planner_run::*, plans::*, profiles::*, projects::*, questions::*, releases::*,
        runner::*, strays::*, system::*, threads::*, tickets::*, usage::*,
    };
    let (router, mut api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(list_profiles))
        .routes(routes!(list_projects, create_project))
        .routes(routes!(get_project, patch_project))
        .routes(routes!(list_designs, upload_design))
        .routes(routes!(add_figma_design))
        .routes(routes!(fetch_design))
        .routes(routes!(design_file))
        .routes(routes!(design_thumb))
        .routes(routes!(rename_design, delete_design))
        .routes(routes!(put_figma_token, delete_figma_token))
        .routes(routes!(list_pins))
        .routes(routes!(answer_pin))
        .routes(routes!(planner_pin))
        .routes(routes!(system_checks))
        .routes(routes!(system_places))
        .routes(routes!(cli_status))
        .routes(routes!(cli_install))
        .routes(routes!(repos_check))
        .routes(routes!(repos_prepare))
        .routes(routes!(list_agents))
        .routes(routes!(get_usage))
        .routes(routes!(get_usage_today))
        .routes(routes!(put_usage_settings))
        .routes(routes!(post_usage_quota))
        .routes(routes!(get_inbox))
        .routes(routes!(get_suggestion))
        .routes(routes!(resolve_leftovers))
        .routes(routes!(create_chain))
        .routes(routes!(import_chain))
        .routes(routes!(export_chain))
        .routes(routes!(list_chains))
        .routes(routes!(get_chain, patch_chain, delete_chain))
        .routes(routes!(move_chain))
        .routes(routes!(add_edge, remove_edge))
        .routes(routes!(move_edge))
        .routes(routes!(move_design))
        .routes(routes!(move_ticket))
        .routes(routes!(arrange_chain))
        .routes(routes!(archive_project))
        .routes(routes!(unarchive_project))
        .routes(routes!(delete_project))
        .routes(routes!(list_strays))
        .routes(routes!(clean_strays))
        .routes(routes!(import_path))
        .routes(routes!(list_plans))
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
        .routes(routes!(post_discussion))
        .routes(routes!(accept_edit))
        .routes(routes!(reject_edit))
        .routes(routes!(get_conversation, post_conversation))
        .routes(routes!(planner_say))
        .routes(routes!(get_planner))
        .routes(routes!(start_planner))
        .routes(routes!(stop_planner))
        .routes(routes!(planner_scrollback))
        .routes(routes!(planner_input))
        .routes(routes!(planner_size))
        .routes(routes!(planner_plan))
        .routes(routes!(planner_overview))
        .routes(routes!(planner_chain))
        .routes(routes!(planner_design))
        .routes(routes!(planner_new_chain))
        .routes(routes!(planner_edit_chain))
        .routes(routes!(planner_add_card))
        .routes(routes!(planner_edit_card, planner_card))
        .routes(routes!(planner_show_card))
        .routes(routes!(planner_suggest))
        .routes(routes!(planner_add_dependency, planner_remove_dependency))
        .routes(routes!(planner_move_card))
        .routes(routes!(planner_open_thread))
        .routes(routes!(planner_thread_message))
        .routes(routes!(planner_thread_version))
        .routes(routes!(scrollback))
        .routes(routes!(runner_input))
        .routes(routes!(runner_answer))
        .routes(routes!(agent_permission))
        .routes(routes!(agent_pre_tool))
        .routes(routes!(runner_size))
        .routes(routes!(release_action))
        .routes(routes!(release_offer))
        .routes(routes!(next_version))
        .routes(routes!(start_release))
        .routes(routes!(release_run))
        .routes(routes!(list_events))
        .routes(routes!(agent_event))
        .routes(routes!(ticket_activity))
        .routes(routes!(claim_instructions))
        .routes(routes!(release_event))
        .split_for_parts();
    // Routes are nested under /v1; say so in the document.
    api.servers = Some(vec![utoipa::openapi::Server::new("/v1")]);
    (router, strict::close_bodies(api))
}

/// The OpenAPI document as JSON.
pub fn openapi_json() -> String {
    v1().1.to_pretty_json().unwrap_or_default()
}

#[cfg(test)]
mod tests;
