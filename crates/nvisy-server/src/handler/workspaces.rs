//! Workspace management handlers for CRUD operations.
//!
//! This module provides comprehensive workspace management functionality including
//! creating, reading, updating, and deleting workspaces. All operations are secured
//! with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;
use nvisy_postgres::PgError;
use nvisy_postgres::model::{NewWorkspaceMember, Workspace as WorkspaceModel, WorkspaceMember};
use nvisy_postgres::query::{WorkspaceMemberRepository, WorkspaceRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{CreateWorkspace, Pagination, UpdateWorkspace, WorkspacePathParams};
use crate::handler::response::{ErrorResponse, Workspace, Workspaces};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspaces";

/// Creates a new workspace with the authenticated user as admin.
///
/// The creator is automatically added as an admin member of the workspace,
/// granting full management permissions.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_workspace(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    ValidateJson(request): ValidateJson<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace");

    let new_workspace = request.into_model(auth_state.account_id);
    let creator_id = auth_state.account_id;

    let (workspace, _membership) = conn
        .transaction(|conn| {
            Box::pin(async move {
                let workspace = conn.create_workspace(new_workspace).await?;
                let new_member = NewWorkspaceMember::new_owner(workspace.id, creator_id);
                let member = conn.add_workspace_member(new_member).await?;
                Ok::<(WorkspaceModel, WorkspaceMember), PgError>((workspace, member))
            })
        })
        .await?;

    let response = Workspace::from_model(workspace);

    tracing::info!(
        target: TRACING_TARGET,
        workspace_id = %response.workspace_id,
        "Workspace created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create workspace")
        .description(
            "Creates a new workspace. The creator is automatically added as an admin member.",
        )
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
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Workspaces>)> {
    let workspace_memberships = conn
        .list_user_workspaces_with_details(auth_state.account_id, pagination.into())
        .await?;

    let workspaces: Workspaces = workspace_memberships
        .into_iter()
        .map(|(workspace, membership)| Workspace::from_model_with_membership(workspace, membership))
        .collect();

    tracing::info!(
        target: TRACING_TARGET,
        workspace_count = workspaces.len(),
        "Workspaces listed",
    );

    Ok((StatusCode::OK, Json(workspaces)))
}

fn list_workspaces_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspaces")
        .description("Returns all workspaces the authenticated user is a member of.")
        .response::<200, Json<Workspaces>>()
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
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<(StatusCode, Json<Workspace>)> {
    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewWorkspace,
        )
        .await?;

    let Some(workspace) = conn.find_workspace_by_id(path_params.workspace_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Workspace not found: {}", path_params.workspace_id))
            .with_resource("workspace"));
    };

    tracing::info!(target: TRACING_TARGET, "Workspace read");

    let workspace = Workspace::from_model(workspace);
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
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<UpdateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace");

    auth_state
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

    let workspace = Workspace::from_model(workspace);
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
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace");

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::DeleteWorkspace,
        )
        .await?;

    // Verify workspace exists before deletion
    if conn
        .find_workspace_by_id(path_params.workspace_id)
        .await?
        .is_none()
    {
        return Err(ErrorKind::NotFound
            .with_message(format!("Workspace not found: {}", path_params.workspace_id))
            .with_resource("workspace"));
    }

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
            "/workspaces/{workspace_id}/",
            get_with(read_workspace, read_workspace_docs)
                .patch_with(update_workspace, update_workspace_docs)
                .delete_with(delete_workspace, delete_workspace_docs),
        )
        .with_path_items(|item| item.tag("Workspaces"))
}
