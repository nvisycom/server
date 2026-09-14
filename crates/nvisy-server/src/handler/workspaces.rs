//! Workspace management handlers for CRUD and activity operations.
//!
//! This module provides comprehensive workspace management functionality including
//! creating, reading, updating, deleting workspaces, and viewing activity logs.
//! All operations are secured with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use uuid::Uuid;

use crate::domain;
use crate::extract::{
    AuthState, Authorized, AvatarUpload, Json, Query, SecurityContext, ValidateJson,
    WorkspaceContext, markers,
};
use crate::handler::request::{
    CreateWorkspace, CursorPagination, UpdateWorkspace, UpdateWorkspaceNotificationSettings,
};
use crate::handler::response::{Page, Workspace, WorkspaceNotificationSettings, WorkspacesPage};
use crate::handler::utility::resolve_account_ref;
use crate::middleware::UploadConfig;
use crate::response::{ErrorResponse, Result};
use crate::service::event::EventOrigin;
use crate::service::{AvatarService, MAX_AVATAR_UPLOAD_BYTES, ServiceState};

/// Tracing target for workspace operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspaces";

/// Creates a new workspace with the authenticated user as owner.
///
/// The creator is automatically added as an owner of the workspace,
/// granting full management permissions.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_workspace(
    State(pg_client): State<PgClient>,
    State(workspaces): State<domain::WorkspaceService>,
    State(upload): State<UploadConfig>,
    auth_state: AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace");

    let creator_id = auth_state.account_id;
    let new_workspace = request.into_model(creator_id)?;

    let created = workspaces
        .create(
            EventOrigin {
                workspace_id: Uuid::nil(),
                account_id: creator_id,
                security: &security,
            },
            new_workspace,
        )
        .await?;

    // The creator is the authenticated caller; resolve their identity directly.
    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, creator_id).await?;
    let response = Workspace::from_model_with_membership(
        created.workspace,
        &created.membership,
        creator,
        upload.max_file_bytes(),
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
    State(workspaces): State<domain::WorkspaceService>,
    State(upload): State<UploadConfig>,
    auth_state: AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspacesPage>)> {
    let page = workspaces
        .list(auth_state.account_id, pagination.into_cursor())
        .await?;

    let hard_max_upload_bytes = upload.max_file_bytes();
    let response = Page::from_cursor_page(page, |(workspace, member, creator)| {
        Workspace::from_model_with_membership(
            workspace,
            &member,
            creator.into(),
            hard_max_upload_bytes,
        )
    });

    tracing::debug!(
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn read_workspace(
    State(pg_client): State<PgClient>,
    State(upload): State<UploadConfig>,
    authz: Authorized<markers::ViewWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    let workspace = authz.workspace;
    let member = authz.member;

    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, workspace.created_by).await?;

    let hard = upload.max_file_bytes();
    let response = Workspace::from_model_with_membership(workspace, &member, creator, hard);
    Ok((StatusCode::OK, Json(response)))
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn update_workspace(
    State(pg_client): State<PgClient>,
    State(workspaces): State<domain::WorkspaceService>,
    State(upload): State<UploadConfig>,
    authz: Authorized<markers::UpdateWorkspace>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let member = authz.member;

    let update_data = request.into_model()?;
    let updated = workspaces
        .update(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            update_data,
        )
        .await?;

    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, updated.created_by).await?;

    let hard = upload.max_file_bytes();
    let response = Workspace::from_model_with_membership(updated, &member, creator, hard);

    Ok((StatusCode::OK, Json(response)))
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn delete_workspace(
    State(workspaces): State<domain::WorkspaceService>,
    authz: Authorized<markers::DeleteWorkspace>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace");

    let account_id = authz.account_id;
    let workspace = authz.workspace;

    workspaces
        .delete(EventOrigin {
            workspace_id: workspace.id,
            account_id,
            security: &security,
        })
        .await?;

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
        workspace_id = %workspace.id,
    )
)]
async fn get_notification_settings(
    State(members): State<domain::WorkspaceMemberService>,
    auth_state: AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<(StatusCode, Json<WorkspaceNotificationSettings>)> {
    let member = members
        .notification_settings(workspace.id, auth_state.account_id)
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceNotificationSettings::from_member(&member)),
    ))
}

fn get_notification_settings_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get notification settings")
        .description("Returns the notification settings for the authenticated user in a workspace.")
        .response::<200, Json<WorkspaceNotificationSettings>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates the notification settings for the authenticated user in a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn update_notification_settings(
    State(members): State<domain::WorkspaceMemberService>,
    auth_state: AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceNotificationSettings>,
) -> Result<(StatusCode, Json<WorkspaceNotificationSettings>)> {
    let member = members
        .update_notification_settings(workspace.id, auth_state.account_id, request.into_model())
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceNotificationSettings::from_member(&member)),
    ))
}

fn update_notification_settings_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update notification settings")
        .description("Updates the notification settings for the authenticated user in a workspace.")
        .response::<200, Json<WorkspaceNotificationSettings>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all workspace-related routes.
///
/// [`Router`]: axum::routing::Router
/// Uploads (or replaces) a workspace's avatar (logo).
///
/// The image is normalized to WebP and stored; the workspace's `avatar_url` is
/// set to its serve path. Requires `UpdateWorkspace`.
#[tracing::instrument(skip_all, fields(account_id = %authz.account_id, workspace_id = %authz.workspace.id))]
async fn upload_workspace_avatar(
    State(avatar): State<AvatarService>,
    authz: Authorized<markers::UpdateWorkspace>,
    AvatarUpload(bytes): AvatarUpload,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Uploading workspace avatar");

    // Authorization is enforced by the `Authorized` extractor. `set_workspace_avatar`
    // does image processing and a NATS put (and acquires its own connection for
    // the DB update).
    let workspace = authz.workspace;
    avatar.set_workspace_avatar(workspace.id, bytes).await?;

    tracing::info!(target: TRACING_TARGET, "Workspace avatar set");
    Ok(StatusCode::OK)
}

fn upload_workspace_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Upload workspace avatar")
        .description("Uploads and normalizes the workspace's avatar. Requires UpdateWorkspace.")
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Removes a workspace's avatar. Requires `UpdateWorkspace`.
#[tracing::instrument(skip_all, fields(account_id = %authz.account_id, workspace_id = %authz.workspace.id))]
async fn delete_workspace_avatar(
    State(avatar): State<AvatarService>,
    authz: Authorized<markers::UpdateWorkspace>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace avatar");

    let workspace = authz.workspace;
    avatar.delete_workspace_avatar(workspace.id).await?;
    tracing::info!(target: TRACING_TARGET, "Workspace avatar deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_workspace_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete workspace avatar")
        .description("Removes the workspace's avatar. Requires UpdateWorkspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with, put_with};

    ApiRouter::new()
        .api_route(
            "/workspaces",
            post_with(create_workspace, create_workspace_docs)
                .get_with(list_workspaces, list_workspaces_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}",
            get_with(read_workspace, read_workspace_docs)
                .patch_with(update_workspace, update_workspace_docs)
                .delete_with(delete_workspace, delete_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/avatar",
            put_with(upload_workspace_avatar, upload_workspace_avatar_docs)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES))
                .delete_with(delete_workspace_avatar, delete_workspace_avatar_docs),
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
