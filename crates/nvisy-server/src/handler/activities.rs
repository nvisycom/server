//! Workspace activity handlers.
//!
//! This module provides handlers for viewing workspace activity logs.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceActivityRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query};
use crate::handler::Result;
use crate::handler::request::{CursorPagination, WorkspacePathParams};
use crate::handler::response::{ActivitiesPage, Activity, ErrorResponse};
use crate::service::ServiceState;

/// Tracing target for activity operations.
const TRACING_TARGET: &str = "nvisy_server::handler::activities";

/// Lists activities for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_activities(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ActivitiesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace activities");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewWorkspace,
        )
        .await?;

    let page = conn
        .cursor_list_workspace_activity(path_params.workspace_id, pagination.into())
        .await?;

    let response = ActivitiesPage::from_cursor_page(page, Activity::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        activity_count = response.items.len(),
        "Workspace activities listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_activities_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace activities")
        .description("Returns all activity log entries for a workspace.")
        .response::<200, Json<ActivitiesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Returns routes for workspace activity management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/activities/",
            get_with(list_activities, list_activities_docs),
        )
        .with_path_items(|item| item.tag("Activities"))
}
