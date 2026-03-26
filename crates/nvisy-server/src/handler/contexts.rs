//! Workspace context file management handlers.
//!
//! Context files are encrypted JSON documents stored in NATS object storage.
//! The metadata (name, size, hash) is stored in PostgreSQL while the actual
//! content is encrypted with workspace-derived keys and stored as objects.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use nvisy_nats::NatsClient;
use nvisy_nats::object::{ContextFilesBucket, ContextKey};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewWorkspaceContext, UpdateWorkspaceContext};
use nvisy_postgres::query::WorkspaceContextRepository;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Multipart, Path, Permission, Query};
use crate::handler::request::{ContextPathParams, CursorPagination, WorkspacePathParams};
use crate::handler::response::{Context, ContextsPage, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::middleware::DEFAULT_MAX_FILE_BODY_SIZE;
use crate::service::crypto::encrypt;
use crate::service::{MasterKey, ServiceState};

/// Tracing target for workspace context operations.
const TRACING_TARGET: &str = "nvisy_server::handler::contexts";

/// Creates a new workspace context from a multipart upload.
///
/// Expects a multipart form with:
/// - `name`: Context name (text field)
/// - `file`: The JSON context file (file field)
/// - `description`: Optional description (text field)
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_context(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(master_key): State<MasterKey>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Multipart(mut multipart): Multipart,
) -> Result<(StatusCode, Json<Context>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace context");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageContexts,
        )
        .await?;

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!(target: TRACING_TARGET, error = %err, "Failed to read multipart field");
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "name" => {
                name = Some(field.text().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read name field")
                        .with_context(err.to_string())
                })?);
            }
            "description" => {
                description = Some(field.text().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read description field")
                        .with_context(err.to_string())
                })?);
            }
            "file" => {
                let bytes = field.bytes().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read file content")
                        .with_context(err.to_string())
                })?;

                // Validate it's valid JSON
                serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Context file must be valid JSON")
                        .with_context(err.to_string())
                })?;

                file_content = Some(bytes.to_vec());
            }
            _ => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    field = %field_name,
                    "Skipping unknown multipart field"
                );
            }
        }
    }

    let name =
        name.ok_or_else(|| ErrorKind::BadRequest.with_message("Missing required 'name' field"))?;
    let content = file_content
        .ok_or_else(|| ErrorKind::BadRequest.with_message("Missing required 'file' field"))?;

    // Encrypt the content with workspace-derived key
    let workspace_key = master_key.derive_workspace_key(path_params.workspace_id);
    let encrypted_content =
        encrypt(&workspace_key, &content).map_err(|e: crate::service::crypto::CryptoError| {
            ErrorKind::InternalServerError
                .with_message("Failed to encrypt context content")
                .with_context(e.to_string())
        })?;

    // Generate the object store key
    let context_id = Uuid::now_v7();
    let context_key = ContextKey::new(path_params.workspace_id, context_id);

    // Store encrypted content in NATS
    let context_store = nats_client
        .object_store::<ContextFilesBucket, ContextKey>()
        .await?;

    let reader = std::io::Cursor::new(encrypted_content);
    let put_result = context_store.put(&context_key, reader).await?;

    tracing::debug!(
        target: TRACING_TARGET,
        context_key = %context_key,
        size = put_result.size(),
        "Context file stored in NATS"
    );

    // Create metadata record in PostgreSQL
    let new_context = NewWorkspaceContext {
        workspace_id: path_params.workspace_id,
        account_id: auth_state.account_id,
        name,
        description,
        mime_type: "application/json".to_string(),
        storage_key: context_key.to_string(),
        content_size: put_result.size() as i64,
        content_hash: put_result.sha256().to_vec(),
        metadata: None,
    };

    let context = conn.create_workspace_context(new_context).await?;

    tracing::info!(
        target: TRACING_TARGET,
        context_id = %context.id,
        "Context created",
    );

    Ok((StatusCode::CREATED, Json(Context::from_model(context))))
}

fn create_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create context")
        .description(
            "Creates a new context file for the workspace via multipart upload. \
             The file content is encrypted and stored in NATS object storage. \
             Expects fields: 'name' (text), 'file' (JSON file), 'description' (optional text).",
        )
        .response::<201, Json<Context>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all contexts for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_contexts(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ContextsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace contexts");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewContexts,
        )
        .await?;

    let page = conn
        .cursor_list_workspace_contexts(path_params.workspace_id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        context_count = page.items.len(),
        "Workspace contexts listed",
    );

    Ok((
        StatusCode::OK,
        Json(ContextsPage::from_cursor_page(page, Context::from_model)),
    ))
}

fn list_contexts_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List contexts")
        .description("Returns all context files for the workspace with metadata.")
        .response::<200, Json<ContextsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace context.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        context_id = %path_params.context_id,
    )
)]
async fn read_context(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
) -> Result<(StatusCode, Json<Context>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace context");

    let mut conn = pg_client.get_connection().await?;

    let context = find_context(&mut conn, path_params.context_id).await?;

    auth_state
        .authorize_workspace(&mut conn, context.workspace_id, Permission::ViewContexts)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace context read");

    Ok((StatusCode::OK, Json(Context::from_model(context))))
}

fn read_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get context")
        .description("Returns context file metadata.")
        .response::<200, Json<Context>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace context.
///
/// Updates context metadata and optionally replaces the content via multipart.
/// Expects a multipart form with optional fields:
/// - `name`: New context name (text field)
/// - `description`: New description (text field)
/// - `file`: Replacement JSON context file (file field)
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        context_id = %path_params.context_id,
    )
)]
async fn update_context(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    State(master_key): State<MasterKey>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
    Multipart(mut multipart): Multipart,
) -> Result<(StatusCode, Json<Context>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace context");

    let mut conn = pg_client.get_connection().await?;

    let existing = find_context(&mut conn, path_params.context_id).await?;

    auth_state
        .authorize_workspace(&mut conn, existing.workspace_id, Permission::ManageContexts)
        .await?;

    let mut name: Option<String> = None;
    let mut description: Option<Option<String>> = None;
    let mut file_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        ErrorKind::BadRequest
            .with_message("Invalid multipart data")
            .with_context(format!("Failed to parse multipart form: {}", err))
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "name" => {
                name = Some(field.text().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read name field")
                        .with_context(err.to_string())
                })?);
            }
            "description" => {
                let text = field.text().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read description field")
                        .with_context(err.to_string())
                })?;
                description = Some(if text.is_empty() { None } else { Some(text) });
            }
            "file" => {
                let bytes = field.bytes().await.map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Failed to read file content")
                        .with_context(err.to_string())
                })?;

                serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Context file must be valid JSON")
                        .with_context(err.to_string())
                })?;

                file_content = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    let mut updates = UpdateWorkspaceContext {
        name,
        description,
        ..Default::default()
    };

    // If file content was provided, encrypt and store new content
    if let Some(content) = file_content {
        let workspace_key = master_key.derive_workspace_key(existing.workspace_id);
        let encrypted_content = encrypt(&workspace_key, &content).map_err(
            |e: crate::service::crypto::CryptoError| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt context content")
                    .with_context(e.to_string())
            },
        )?;

        let context_key = ContextKey::new(existing.workspace_id, existing.id);
        let context_store = nats_client
            .object_store::<ContextFilesBucket, ContextKey>()
            .await?;

        let reader = std::io::Cursor::new(encrypted_content);
        let put_result = context_store.put(&context_key, reader).await?;

        updates.storage_key = Some(context_key.to_string());
        updates.content_size = Some(put_result.size() as i64);
        updates.content_hash = Some(put_result.sha256().to_vec());
    }

    let context = conn
        .update_workspace_context(path_params.context_id, updates)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Context updated");

    Ok((StatusCode::OK, Json(Context::from_model(context))))
}

fn update_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update context")
        .description(
            "Updates context metadata and optionally replaces the content via multipart upload. \
             All fields are optional. If a 'file' field is provided, the content is re-encrypted \
             and stored.",
        )
        .response::<200, Json<Context>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace context.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        context_id = %path_params.context_id,
    )
)]
async fn delete_context(
    State(pg_client): State<PgClient>,
    State(nats_client): State<NatsClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace context");

    let mut conn = pg_client.get_connection().await?;

    let context = find_context(&mut conn, path_params.context_id).await?;

    auth_state
        .authorize_workspace(&mut conn, context.workspace_id, Permission::ManageContexts)
        .await?;

    // Delete the object from NATS (best effort, context may already be gone)
    let context_store = nats_client
        .object_store::<ContextFilesBucket, ContextKey>()
        .await?;
    let context_key = ContextKey::new(context.workspace_id, context.id);
    if let Err(err) = context_store.delete(&context_key).await {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            context_id = %path_params.context_id,
            "Failed to delete context object from NATS (proceeding with soft delete)"
        );
    }

    conn.delete_workspace_context(path_params.context_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Context deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete context")
        .description("Soft-deletes the context from the workspace and removes the encrypted content from NATS.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a context by ID or returns NotFound error.
async fn find_context(
    conn: &mut nvisy_postgres::PgConn,
    context_id: Uuid,
) -> Result<nvisy_postgres::model::WorkspaceContext> {
    conn.find_workspace_context_by_id(context_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Context not found")
                .with_resource("context")
        })
}

/// Returns routes for workspace context management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/contexts/",
            post_with(create_context, create_context_docs)
                .layer(DefaultBodyLimit::max(DEFAULT_MAX_FILE_BODY_SIZE))
                .get_with(list_contexts, list_contexts_docs),
        )
        .api_route(
            "/contexts/{contextId}/",
            get_with(read_context, read_context_docs)
                .put_with(update_context, update_context_docs)
                .layer(DefaultBodyLimit::max(DEFAULT_MAX_FILE_BODY_SIZE))
                .delete_with(delete_context, delete_context_docs),
        )
        .with_path_items(|item| item.tag("Contexts"))
}
