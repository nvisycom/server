//! Workspace management handlers for CRUD and activity operations.
//!
//! This module provides comprehensive workspace management functionality including
//! creating, reading, updating, deleting workspaces, and viewing activity logs.
//! All operations are secured with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceMember, Workspace as WorkspaceModel, WorkspaceMember};
use nvisy_postgres::query::{
    WorkspaceFileRepository, WorkspaceMemberRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{FileKind, RetentionScope, RetentionSettings};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Avatar, Json, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{
    CreateWorkspace, CursorPagination, UpdateNotificationSettings, UpdateWorkspace,
};
use crate::handler::response::{
    AccountRef, ErrorResponse, NotificationSettings, Page, Workspace, WorkspacesPage,
};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, ErrorKind, Result};
use crate::middleware::UploadConfig;
use crate::service::{
    AvatarService, EventEmitter, EventOrigin, MAX_AVATAR_UPLOAD_BYTES, ServiceState,
    WorkspaceEvent, WorkspaceRef,
};

/// Tracing target for workspace operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspaces";

/// Recomputes each document scope's `expires_at` on a workspace's existing live
/// files after its retention settings change, so a change takes effect on data
/// already stored (not just future writes). This applies the workspace baseline
/// across all files of each kind; a pipeline's own override backfill is handled
/// separately when that pipeline is updated (see `update_pipeline`).
async fn backfill_retention(
    conn: &mut PgConn,
    workspace_id: Uuid,
    retention: &RetentionSettings,
) -> Result<()> {
    let now = jiff::Timestamp::now();
    for (scope, kind) in [
        (RetentionScope::OriginalDocuments, FileKind::Original),
        (RetentionScope::RedactedDocuments, FileKind::Redacted),
        (RetentionScope::AuditLogs, FileKind::Audit),
        // Review audits share the audit-logs scope with detection audits; a
        // redaction stages them under `AuditLogs`, so they backfill under it too.
        (RetentionScope::AuditLogs, FileKind::Review),
        (RetentionScope::Intermediates, FileKind::Intermediate),
    ] {
        let expires_at = retention.get(scope).expires_at(now);
        conn.backfill_files_expiry(workspace_id, kind, expires_at)
            .await?;
    }
    Ok(())
}

/// Creates a new workspace with the authenticated user as owner.
///
/// The creator is automatically added as an owner of the workspace,
/// granting full management permissions.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_workspace(
    State(pg_client): State<PgClient>,
    State(upload): State<UploadConfig>,
    AuthState(auth_state): AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace");

    let new_workspace = request.into_model(auth_state.account_id)?;
    let mut conn = pg_client.get_connection().await?;
    let creator_id = auth_state.account_id;

    // The workspace, its owner membership, and the creation event commit
    // together, so the event is never lost, nor recorded for a workspace that
    // rolled back.
    let (workspace, membership) = conn
        .transaction(async |conn| {
            let workspace = conn.create_workspace(new_workspace).await?;
            let new_member = NewWorkspaceMember::new_owner(workspace.id, creator_id);
            let member = conn.add_workspace_member(new_member).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: creator_id,
                    security: &security,
                },
                WorkspaceEvent::WorkspaceCreated(WorkspaceRef {
                    workspace_id: workspace.id,
                    workspace_slug: workspace.slug.clone(),
                }),
            )
            .await?;
            Ok::<(WorkspaceModel, WorkspaceMember), Error>((workspace, member))
        })
        .await?;

    // The creator is the authenticated caller; resolve their identity directly.
    let creator = resolve_account_ref(&mut conn, creator_id).await?;
    let response = Workspace::from_model_with_membership(
        workspace,
        membership,
        creator,
        upload.max_file_bytes(),
    );

    tracing::info!(
        target: TRACING_TARGET,
        workspace_slug = %response.slug,
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
    State(upload): State<UploadConfig>,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspacesPage>)> {
    let mut conn = pg_client.get_connection().await?;
    let page = conn
        .cursor_list_account_workspaces_with_details(auth_state.account_id, pagination.into())
        .await?;

    let hard_max_upload_bytes = upload.max_file_bytes();
    let response = Page::from_cursor_page(page, |(workspace, member, creator)| {
        Workspace::from_model_with_membership(
            workspace,
            member,
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn read_workspace(
    State(pg_client): State<PgClient>,
    State(upload): State<UploadConfig>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<(StatusCode, Json<Workspace>)> {
    let mut conn = pg_client.get_connection().await?;
    let member = auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWorkspace)
        .await?;

    let creator = find_workspace_creator(&mut conn, workspace.slug.as_str()).await?;

    tracing::info!(target: TRACING_TARGET, "Workspace read");

    let hard = upload.max_file_bytes();
    let response = match member {
        Some(member) => Workspace::from_model_with_membership(workspace, member, creator, hard),
        None => Workspace::from_model(workspace, creator, hard),
    };
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn update_workspace(
    State(pg_client): State<PgClient>,
    State(upload): State<UploadConfig>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace");

    let mut conn = pg_client.get_connection().await?;
    let member = auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdateWorkspace)
        .await?;

    // Capture the new retention so, if settings changed, we can backfill the
    // precomputed `expires_at` on existing files for this workspace.
    let new_retention = request.settings.map(|settings| settings.retention);

    let update_data = request.into_model()?;

    // The settings write, the retention backfill, and the update event must be
    // atomic: otherwise a mid-operation failure could persist the new settings
    // while existing files keep stale `expires_at`, update only some file kinds,
    // or record the event out of step with the update.
    let workspace_id = workspace.id;
    let updated = conn
        .transaction(async |conn| {
            let updated = conn.update_workspace(workspace_id, update_data).await?;
            if let Some(retention) = new_retention {
                backfill_retention(conn, workspace_id, &retention).await?;
            }
            conn.emit_event(
                EventOrigin {
                    workspace_id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::WorkspaceUpdated(WorkspaceRef {
                    workspace_id: updated.id,
                    workspace_slug: updated.slug.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(updated)
        })
        .await?;

    let creator = find_workspace_creator(&mut conn, updated.slug.as_str()).await?;

    tracing::info!(target: TRACING_TARGET, "Workspace updated");

    let hard = upload.max_file_bytes();
    let response = match member {
        Some(member) => Workspace::from_model_with_membership(updated, member, creator, hard),
        None => Workspace::from_model(updated, creator, hard),
    };

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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn delete_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::DeleteWorkspace)
        .await?;

    // Soft-delete the workspace and record the deletion event in one transaction,
    // so the event is never lost, nor recorded for a delete that rolled back.
    conn.transaction(async |conn| {
        conn.delete_workspace(workspace.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::WorkspaceDeleted(WorkspaceRef {
                workspace_id: workspace.id,
                workspace_slug: workspace.slug.clone(),
            }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;

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
        workspace_id = %workspace.id,
    )
)]
async fn get_notification_settings(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<(StatusCode, Json<NotificationSettings>)> {
    let mut conn = pg_client.get_connection().await?;
    let Some(member) = conn
        .find_workspace_member(workspace.id, auth_state.account_id)
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
        workspace_id = %workspace.id,
    )
)]
async fn update_notification_settings(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<UpdateNotificationSettings>,
) -> Result<(StatusCode, Json<NotificationSettings>)> {
    let mut conn = pg_client.get_connection().await?;

    // Verify membership exists
    if conn
        .find_workspace_member(workspace.id, auth_state.account_id)
        .await?
        .is_none()
    {
        return Err(ErrorKind::NotFound
            .with_message("Workspace membership not found")
            .with_resource("workspace_member"));
    }

    let update_data = request.into_model();
    let member = conn
        .update_workspace_member(workspace.id, auth_state.account_id, update_data)
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

/// Returns the public identity of the account that created the workspace
/// addressed by `slug`, or a NotFound error if no such workspace exists.
async fn find_workspace_creator(conn: &mut PgConn, slug: &str) -> Result<AccountRef> {
    conn.find_workspace_by_slug(slug)
        .await?
        .map(|wc| wc.account.into())
        .ok_or_else(|| Error::not_found("workspace"))
}

/// Returns a [`Router`] with all workspace-related routes.
///
/// [`Router`]: axum::routing::Router
/// Uploads (or replaces) a workspace's avatar (logo).
///
/// The image is normalized to WebP and stored; the workspace's `avatar_url` is
/// set to its serve path. Requires `UpdateWorkspace`.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id))]
async fn upload_workspace_avatar(
    State(pg_client): State<PgClient>,
    State(avatar): State<AvatarService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Avatar(bytes): Avatar,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Uploading workspace avatar");

    // Authorize under a scoped connection, then release it: `set_workspace_avatar`
    // does image processing and a NATS put (and acquires its own connection for
    // the DB update), so holding this one across it would pin two pooled
    // connections for the whole upload.
    {
        let mut conn = pg_client.get_connection().await?;
        auth_state
            .authorize_workspace(&mut conn, workspace.id, Permission::UpdateWorkspace)
            .await?;
    }

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
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id, workspace_id = %workspace.id))]
async fn delete_workspace_avatar(
    State(pg_client): State<PgClient>,
    State(avatar): State<AvatarService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace avatar");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdateWorkspace)
        .await?;

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
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/",
            post_with(create_workspace, create_workspace_docs)
                .get_with(list_workspaces, list_workspaces_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/",
            get_with(read_workspace, read_workspace_docs)
                .patch_with(update_workspace, update_workspace_docs)
                .delete_with(delete_workspace, delete_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/avatar/",
            put_with(upload_workspace_avatar, upload_workspace_avatar_docs)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES))
                .delete_with(delete_workspace_avatar, delete_workspace_avatar_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/notifications/",
            get_with(get_notification_settings, get_notification_settings_docs).patch_with(
                update_notification_settings,
                update_notification_settings_docs,
            ),
        )
        .with_path_items(|item| item.tag("Workspaces"))
}
