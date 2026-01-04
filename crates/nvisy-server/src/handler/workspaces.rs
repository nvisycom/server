//! Workspace management handlers for CRUD operations.
//!
//! This module provides comprehensive workspace management functionality including
//! creating, reading, updating, and deleting workspaces. All operations are secured
//! with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceMember, Workspace as WorkspaceModel, WorkspaceMember};
use nvisy_postgres::query::{WorkspaceMemberRepository, WorkspaceRepository};
use nvisy_postgres::{PgClient, PgError};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    CreateWorkspace, CursorPagination, UpdateNotificationSettings, UpdateWorkspace,
    WorkspacePathParams,
};
use crate::handler::response::{
    ErrorResponse, NotificationSettings, Page, Workspace, WorkspacesPage,
};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspaces";

/// Creates a new workspace with the authenticated user as owner.
///
/// The creator is automatically added as an owner of the workspace,
/// granting full management permissions.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    ValidateJson(request): ValidateJson<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace");

    let new_workspace = request.into_model(auth_state.account_id);
    let mut conn = pg_client.get_connection().await?;
    let creator_id = auth_state.account_id;

    let (workspace, membership) = conn
        .transaction(|conn| {
            Box::pin(async move {
                let workspace = conn.create_workspace(new_workspace).await?;
                let new_member = NewWorkspaceMember::new_owner(workspace.id, creator_id);
                let member = conn.add_workspace_member(new_member).await?;
                Ok::<(WorkspaceModel, WorkspaceMember), PgError>((workspace, member))
            })
        })
        .await?;

    let response = Workspace::from_model_with_membership(workspace, membership);

    tracing::info!(
        target: TRACING_TARGET,
        workspace_id = %response.workspace_id,
        "Workspace created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create workspace")
        .description("Creates a new workspace. The creator is automatically added as an owner.")
        .response::<201, Json<Workspace>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Lists all workspaces the authenticated user is a member of.
///
/// Returns workspaces with membership details including the user's role
/// in each workspace.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_workspaces(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspacesPage>)> {
    let mut conn = pg_client.get_connection().await?;
    let page = conn
        .cursor_list_account_workspaces_with_details(auth_state.account_id, pagination.into())
        .await?;

    let response = Page::from_cursor_page(page, |(workspace, member)| {
        Workspace::from_model_with_membership(workspace, member)
    });

    tracing::info!(
        target: TRACING_TARGET,
        workspace_count = response.items.len(),
        "Workspaces listed",
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_workspaces_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspaces")
        .description("Returns all workspaces the authenticated user is a member of.")
        .response::<200, Json<WorkspacesPage>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Retrieves details for a specific workspace.
///
/// Requires `ViewWorkspace` permission for the requested workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn read_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<(StatusCode, Json<Workspace>)> {
    let mut conn = pg_client.get_connection().await?;
    let member = auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewWorkspace,
        )
        .await?;

    let Some(workspace) = conn.find_workspace_by_id(path_params.workspace_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message("Workspace not found")
            .with_resource("workspace"));
    };

    tracing::info!(target: TRACING_TARGET, "Workspace read");

    let workspace = match member {
        Some(member) => Workspace::from_model_with_membership(workspace, member),
        None => Workspace::from_model(workspace),
    };
    Ok((StatusCode::OK, Json(workspace)))
}

fn read_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get workspace")
        .description("Returns details for a specific workspace.")
        .response::<200, Json<Workspace>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing workspace's configuration.
///
/// Requires `UpdateWorkspace` permission. Only provided fields are updated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn update_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace");

    let mut conn = pg_client.get_connection().await?;
    let member = auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::UpdateWorkspace,
        )
        .await?;

    let update_data = request.into_model();
    let workspace = conn
        .update_workspace(path_params.workspace_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Workspace updated");

    let workspace = match member {
        Some(member) => Workspace::from_model_with_membership(workspace, member),
        None => Workspace::from_model(workspace),
    };
    Ok((StatusCode::OK, Json(workspace)))
}

fn update_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update workspace")
        .description(
            "Updates an existing workspace's configuration. Only provided fields are updated.",
        )
        .response::<200, Json<Workspace>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Soft-deletes a workspace.
///
/// Requires `DeleteWorkspace` permission. The workspace is marked as deleted
/// but data is retained for potential recovery.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn delete_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::DeleteWorkspace,
        )
        .await?;

    conn.delete_workspace(path_params.workspace_id).await?;

    tracing::info!(target: TRACING_TARGET, "Workspace deleted");

    Ok(StatusCode::OK)
}

fn delete_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete workspace")
        .description("Soft-deletes a workspace. Data is retained for potential recovery.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Retrieves the notification settings for the authenticated user in a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn get_notification_settings(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<(StatusCode, Json<NotificationSettings>)> {
    let mut conn = pg_client.get_connection().await?;
    let Some(member) = conn
        .find_workspace_member(path_params.workspace_id, auth_state.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Workspace membership not found")
            .with_resource("workspace_member"));
    };

    tracing::debug!(target: TRACING_TARGET, "Notification settings retrieved");

    Ok((
        StatusCode::OK,
        Json(NotificationSettings::from_member(&member)),
    ))
}

fn get_notification_settings_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get notification settings")
        .description("Returns the notification settings for the authenticated user in a workspace.")
        .response::<200, Json<NotificationSettings>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates the notification settings for the authenticated user in a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn update_notification_settings(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<UpdateNotificationSettings>,
) -> Result<(StatusCode, Json<NotificationSettings>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify membership exists
    if conn
        .find_workspace_member(path_params.workspace_id, auth_state.account_id)
        .await?
        .is_none()
    {
        return Err(ErrorKind::NotFound
            .with_message("Workspace membership not found")
            .with_resource("workspace_member"));
    }

    let update_data = request.into_model();
    let member = conn
        .update_workspace_member(path_params.workspace_id, auth_state.account_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Notification settings updated");

    Ok((
        StatusCode::OK,
        Json(NotificationSettings::from_member(&member)),
    ))
}

fn update_notification_settings_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update notification settings")
        .description("Updates the notification settings for the authenticated user in a workspace.")
        .response::<200, Json<NotificationSettings>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all workspace-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/",
            post_with(create_workspace, create_workspace_docs)
                .get_with(list_workspaces, list_workspaces_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/",
            get_with(read_workspace, read_workspace_docs)
                .patch_with(update_workspace, update_workspace_docs)
                .delete_with(delete_workspace, delete_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/notifications",
            get_with(get_notification_settings, get_notification_settings_docs).patch_with(
                update_notification_settings,
                update_notification_settings_docs,
            ),
        )
        .with_path_items(|item| item.tag("Workspaces"))
}
