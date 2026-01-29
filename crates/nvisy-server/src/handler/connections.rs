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
use nvisy_core::crypto::encrypt_json;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewWorkspaceConnection, UpdateWorkspaceConnection};
use nvisy_postgres::query::WorkspaceConnectionRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    ConnectionPathParams, ConnectionsQuery, CreateConnection, CursorPagination, UpdateConnection,
    WorkspacePathParams,
};
use crate::handler::response::{Connection, ConnectionsPage, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::service::{MasterKey, ServiceState};

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
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_connection(
    State(pg_client): State<PgClient>,
    State(master_key): State<MasterKey>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageConnections,
        )
        .await?;

    let workspace_key = master_key.derive_workspace_key(path_params.workspace_id);
    let encrypted_data = encrypt_json(&workspace_key, &request.data).map_err(|e| {
        ErrorKind::InternalServerError
            .with_message("Failed to encrypt connection data")
            .with_context(e.to_string())
    })?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: path_params.workspace_id,
        account_id: auth_state.account_id,
        name: request.name,
        provider: request.provider,
        encrypted_data,
        is_active: None,
        metadata: None,
    };

    let connection = conn.create_workspace_connection(new_connection).await?;

    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %connection.id,
        provider = %connection.provider,
        "Connection created",
    );

    Ok((
        StatusCode::CREATED,
        Json(Connection::from_model(connection)),
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
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_connections(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<ConnectionsQuery>,
) -> Result<(StatusCode, Json<ConnectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connections");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewConnections,
        )
        .await?;

    let page = conn
        .cursor_list_workspace_connections(
            path_params.workspace_id,
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
            Connection::from_model,
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
        connection_id = %path_params.connection_id,
    )
)]
async fn read_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace connection");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the connection first to get workspace context for authorization
    let connection = find_connection(&mut conn, path_params.connection_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            connection.workspace_id,
            Permission::ViewConnections,
        )
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace connection read");

    Ok((StatusCode::OK, Json(Connection::from_model(connection))))
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
        connection_id = %path_params.connection_id,
    )
)]
async fn update_connection(
    State(pg_client): State<PgClient>,
    State(master_key): State<MasterKey>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<UpdateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the connection first to get workspace context for authorization
    let existing = find_connection(&mut conn, path_params.connection_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::ManageConnections,
        )
        .await?;

    let encrypted_data = request
        .data
        .map(|data| {
            let workspace_key = master_key.derive_workspace_key(existing.workspace_id);
            encrypt_json(&workspace_key, &data).map_err(|e| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt connection data")
                    .with_context(e.to_string())
            })
        })
        .transpose()?;

    let update_data = UpdateWorkspaceConnection {
        name: request.name,
        encrypted_data,
        ..Default::default()
    };

    let connection = conn
        .update_workspace_connection(path_params.connection_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Connection updated");

    Ok((StatusCode::OK, Json(Connection::from_model(connection))))
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
        connection_id = %path_params.connection_id,
    )
)]
async fn delete_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace connection");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the connection first to get workspace context for authorization
    let connection = find_connection(&mut conn, path_params.connection_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            connection.workspace_id,
            Permission::ManageConnections,
        )
        .await?;

    conn.delete_workspace_connection(path_params.connection_id)
        .await?;

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

/// Finds a connection by ID or returns NotFound error.
async fn find_connection(
    conn: &mut nvisy_postgres::PgConn,
    connection_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceConnection> {
    conn.find_workspace_connection_by_id(connection_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Connection not found")
                .with_resource("connection")
        })
}

/// Returns routes for workspace connection management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceId}/connections/",
            post_with(create_connection, create_connection_docs)
                .get_with(list_connections, list_connections_docs),
        )
        // Connection-specific routes (connection ID is globally unique)
        .api_route(
            "/connections/{connectionId}/",
            get_with(read_connection, read_connection_docs)
                .put_with(update_connection, update_connection_docs)
                .delete_with(delete_connection, delete_connection_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
