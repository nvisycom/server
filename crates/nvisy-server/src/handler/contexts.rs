//! Workspace context management handlers.
//!
//! Contexts are structured reference-data documents (the engine's Context
//! type) consumed by the redaction pipeline. The definition is validated
//! against the schema, then stored encrypted (XChaCha20-Poly1305, workspace-
//! derived key) as a JSONB-free BYTEA column in PostgreSQL, scoped to a
//! workspace.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceContext, UpdateWorkspaceContext, WorkspaceContext};
use nvisy_postgres::query::WorkspaceContextRepository;
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    ContextPathParams, CreateContext, CursorPagination, UpdateContext, WorkspacePathParams,
};
use crate::handler::response::{Context, ContextsPage, ErrorResponse};
use crate::handler::{Error, Result};
use crate::service::{CryptoService, ServiceState};

/// Tracing target for workspace context operations.
const TRACING_TARGET: &str = "nvisy_server::handler::contexts";

/// Creates a new workspace context.
///
/// The request body carries a structured context definition; its name,
/// description, and version drive the stored record unless overridden.
/// Requires `ManageContexts` permission for the workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_context(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateContext>,
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

    let definition = &request.definition;
    let name = request.name.unwrap_or_else(|| definition.name.clone());
    let description = request
        .description
        .or_else(|| definition.description.clone());
    let version = definition.version.to_string();
    let encrypted = crypto.encrypt_json(path_params.workspace_id, definition)?;

    let new_context = NewWorkspaceContext {
        workspace_id: path_params.workspace_id,
        account_id: auth_state.account_id,
        name,
        description,
        version,
        definition: encrypted,
        metadata: None,
    };

    let context = conn.create_workspace_context(new_context).await?;

    tracing::info!(target: TRACING_TARGET, context_id = %context.id, "Context created");

    Ok((
        StatusCode::CREATED,
        Json(Context::from_model(context, &crypto)?),
    ))
}

fn create_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create context")
        .description("Creates a structured reference-data context for the workspace.")
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
    State(crypto): State<CryptoService>,
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

    let page =
        ContextsPage::try_from_cursor_page(page, |model| Context::from_model(model, &crypto))?;

    Ok((StatusCode::OK, Json(page)))
}

fn list_contexts_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List contexts")
        .description("Returns all contexts for the workspace.")
        .response::<200, Json<ContextsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace context.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        context_id = %path_params.context_id,
    )
)]
async fn read_context(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
) -> Result<(StatusCode, Json<Context>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace context");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewContexts,
        )
        .await?;

    let context = find_context(&mut conn, path_params.workspace_id, path_params.context_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace context read");

    Ok((StatusCode::OK, Json(Context::from_model(context, &crypto)?)))
}

fn read_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get context")
        .description("Returns a single context.")
        .response::<200, Json<Context>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace context.
///
/// All fields are optional; replacing the definition replaces the whole
/// context body (and its version). Requires `ManageContexts` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        context_id = %path_params.context_id,
    )
)]
async fn update_context(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
    ValidateJson(request): ValidateJson<UpdateContext>,
) -> Result<(StatusCode, Json<Context>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace context");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageContexts,
        )
        .await?;

    // Confirm the context exists in this workspace before mutating.
    find_context(&mut conn, path_params.workspace_id, path_params.context_id).await?;

    let (version, definition) = match &request.definition {
        Some(definition) => {
            let encrypted = crypto.encrypt_json(path_params.workspace_id, definition)?;
            (Some(definition.version.to_string()), Some(encrypted))
        }
        None => (None, None),
    };

    let updates = UpdateWorkspaceContext {
        name: request.name,
        description: request.description,
        version,
        definition,
        ..Default::default()
    };

    let context = conn
        .update_workspace_context(path_params.context_id, updates)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Context updated");

    Ok((StatusCode::OK, Json(Context::from_model(context, &crypto)?)))
}

fn update_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update context")
        .description("Updates context fields. Replacing the definition replaces the whole body.")
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
        workspace_id = %path_params.workspace_id,
        context_id = %path_params.context_id,
    )
)]
async fn delete_context(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ContextPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace context");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageContexts,
        )
        .await?;

    // Confirm the context exists in this workspace before deleting.
    find_context(&mut conn, path_params.workspace_id, path_params.context_id).await?;

    conn.delete_workspace_context(path_params.context_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Context deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_context_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete context")
        .description("Soft-deletes the context from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a context within a workspace or returns NotFound error.
async fn find_context(
    conn: &mut PgConn,
    workspace_id: Uuid,
    context_id: Uuid,
) -> Result<WorkspaceContext> {
    conn.find_context_in_workspace(workspace_id, context_id)
        .await?
        .ok_or_else(|| Error::not_found("context"))
}

/// Returns routes for workspace context management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/contexts/",
            post_with(create_context, create_context_docs)
                .get_with(list_contexts, list_contexts_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/contexts/{contextId}/",
            get_with(read_context, read_context_docs)
                .put_with(update_context, update_context_docs)
                .delete_with(delete_context, delete_context_docs),
        )
        .with_path_items(|item| item.tag("Contexts"))
}
