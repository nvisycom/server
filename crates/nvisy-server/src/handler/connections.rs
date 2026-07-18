//! Workspace connection management handlers.
//!
//! This module provides workspace connection management functionality,
//! allowing workspace members to create, configure, and manage encrypted
//! provider connections. All operations are secured with proper authorization
//! and follow role-based access control principles.
//!
//! # Encryption
//!
//! Connection data (credentials + context) is encrypted using workspace-derived
//! keys (HKDF-SHA256 with XChaCha20-Poly1305). The encrypted data is stored in
//! the database and never exposed through the API.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{
    NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection,
};
use nvisy_postgres::query::WorkspaceConnectionRepository;
use nvisy_postgres::types::Username;
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    ConnectionPathParams, ConnectionsQuery, CreateConnection, CursorPagination, UpdateConnection,
};
use crate::handler::response::{Connection, ConnectionsPage, ErrorResponse};
use crate::handler::{Error, Result};
use crate::service::{CryptoService, ServiceState};

/// Tracing target for workspace connection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connections";

/// Creates a new workspace connection.
///
/// Returns the connection metadata (without encrypted data). Requires
/// `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn create_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<CreateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let encrypted_data = crypto.encrypt_json(workspace.id, &request.data)?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
        slug: request.slug,
        name: request.name,
        provider: request.provider,
        encrypted_data,
        is_active: None,
        metadata: None,
    };

    let connection = conn.create_workspace_connection(new_connection).await?;

    tracing::info!(
        target: TRACING_TARGET,
        connection_slug = %connection.slug,
        provider = %connection.provider,
        "Connection created",
    );

    let (connection, creator_username) =
        find_connection(&mut conn, workspace.id, connection.slug.as_str()).await?;

    Ok((
        StatusCode::CREATED,
        Json(Connection::from_model(
            connection,
            workspace.slug,
            creator_username,
        )),
    ))
}

fn create_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create connection")
        .description(
            "Creates a new provider connection for the workspace. Connection data is encrypted \
             and stored securely. The response includes connection metadata but never exposes \
             the encrypted credentials.",
        )
        .response::<201, Json<Connection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all connections for a workspace.
///
/// Returns connection metadata (without encrypted data). Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_connections(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<ConnectionsQuery>,
) -> Result<(StatusCode, Json<ConnectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connections");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let page = conn
        .cursor_list_workspace_connections(
            workspace.id,
            pagination.into(),
            query.provider.as_deref(),
        )
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        connection_count = page.items.len(),
        "Workspace connections listed",
    );

    Ok((
        StatusCode::OK,
        Json(ConnectionsPage::from_cursor_page(
            page,
            |(connection, creator_username)| {
                Connection::from_model(connection, workspace.slug.clone(), creator_username)
            },
        )),
    ))
}

fn list_connections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connections")
        .description(
            "Returns all configured connections for the workspace. Only metadata is returned; \
             encrypted credentials are never exposed.",
        )
        .response::<200, Json<ConnectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace connection.
///
/// Returns connection metadata (without encrypted data). Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_slug = %path_params.connection_slug,
    )
)]
async fn read_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let (connection, creator_username) =
        find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace connection read");

    Ok((
        StatusCode::OK,
        Json(Connection::from_model(
            connection,
            workspace.slug,
            creator_username,
        )),
    ))
}

fn read_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection")
        .description("Returns connection metadata without encrypted credentials.")
        .response::<200, Json<Connection>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace connection.
///
/// Updates connection configuration. Requires `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_slug = %path_params.connection_slug,
    )
)]
async fn update_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<UpdateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let (existing, _) =
        find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

    let encrypted_data = request
        .data
        .map(|data| crypto.encrypt_json(workspace.id, &data))
        .transpose()?;

    let update_data = UpdateWorkspaceConnection {
        name: request.name,
        encrypted_data,
        ..Default::default()
    };

    conn.update_workspace_connection(existing.id, update_data)
        .await?;

    let (connection, creator_username) =
        find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

    tracing::info!(target: TRACING_TARGET, "Connection updated");

    Ok((
        StatusCode::OK,
        Json(Connection::from_model(
            connection,
            workspace.slug,
            creator_username,
        )),
    ))
}

fn update_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update connection")
        .description("Updates connection name or encrypted data.")
        .response::<200, Json<Connection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace connection.
///
/// Soft-deletes the connection. Requires `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_slug = %path_params.connection_slug,
    )
)]
async fn delete_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let (existing, _) =
        find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

    conn.delete_workspace_connection(existing.id).await?;

    tracing::info!(target: TRACING_TARGET, "Connection deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete connection")
        .description("Soft-deletes the connection from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a connection within a workspace by slug, with its creator's handle, or
/// returns a NotFound error.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_slug: &str,
) -> Result<(WorkspaceConnection, Username)> {
    conn.find_connection_in_workspace_by_slug(workspace_id, connection_slug)
        .await?
        .ok_or_else(|| Error::not_found("connection"))
}

/// Returns routes for workspace connection management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/connections/",
            post_with(create_connection, create_connection_docs)
                .get_with(list_connections, list_connections_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionSlug}/",
            get_with(read_connection, read_connection_docs)
                .put_with(update_connection, update_connection_docs)
                .delete_with(delete_connection, delete_connection_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
