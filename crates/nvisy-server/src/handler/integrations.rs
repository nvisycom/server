//! Workspace integration management handlers.
//!
//! This module provides comprehensive workspace integration management functionality,
//! allowing workspace administrators to create, configure, and manage integrations
//! with external services. All operations are secured with proper authorization
//! and follow role-based access control principles.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceIntegrationRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    CreateIntegration, CursorPagination, IntegrationPathParams, UpdateIntegration,
    UpdateIntegrationCredentials, WorkspacePathParams,
};
use crate::handler::response::{ErrorResponse, Integration, IntegrationsPage};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace integration operations.
const TRACING_TARGET: &str = "nvisy_server::handler::integrations";

/// Creates a new workspace integration.
///
/// Creates an integration with an external service. Requires `ManageIntegrations`
/// permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        integration_type = ?request.integration_type,
    )
)]
async fn create_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateIntegration>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace integration");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let new_integration = request.into_model(path_params.workspace_id, auth_state.account_id);
    let integration = conn.create_workspace_integration(new_integration).await?;

    tracing::info!(
        target: TRACING_TARGET,
        integration_id = %integration.id,
        "Integration created ",
    );

    Ok((
        StatusCode::CREATED,
        Json(Integration::from_model(integration)),
    ))
}

fn create_integration_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create integration")
        .description("Creates a new integration with an external service for the workspace.")
        .response::<201, Json<Integration>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Lists all integrations for a workspace.
///
/// Returns all configured integrations. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_integrations(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<IntegrationsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace integrations");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let page = conn
        .cursor_list_workspace_integrations(path_params.workspace_id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        integration_count = page.items.len(),
        "Workspace integrations listed",
    );

    Ok((
        StatusCode::OK,
        Json(IntegrationsPage::from_cursor_page(
            page,
            Integration::from_model,
        )),
    ))
}

fn list_integrations_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List integrations")
        .description("Returns all configured integrations for the workspace.")
        .response::<200, Json<IntegrationsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace integration.
///
/// Returns integration details. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn read_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace integration");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the integration first to get workspace context for authorization
    let integration = find_integration(&mut conn, path_params.integration_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            integration.workspace_id,
            Permission::ViewIntegrations,
        )
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace integration read");

    Ok((StatusCode::OK, Json(Integration::from_model(integration))))
}

fn read_integration_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get integration")
        .description("Returns details for a specific workspace integration.")
        .response::<200, Json<Integration>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace integration.
///
/// Updates integration configuration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn update_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(request): ValidateJson<UpdateIntegration>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace integration");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the integration first to get workspace context for authorization
    let existing = find_integration(&mut conn, path_params.integration_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let integration = conn
        .update_workspace_integration(path_params.integration_id, request.into_model())
        .await?;

    tracing::info!(target: TRACING_TARGET, "Integration updated");
    Ok((StatusCode::OK, Json(Integration::from_model(integration))))
}

fn update_integration_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update integration")
        .description("Updates integration configuration such as name or settings.")
        .response::<200, Json<Integration>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Updates integration credentials.
///
/// Updates only the authentication credentials. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn update_integration_credentials(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(request): ValidateJson<UpdateIntegrationCredentials>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating integration credentials");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the integration first to get workspace context for authorization
    let existing = find_integration(&mut conn, path_params.integration_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let changes = nvisy_postgres::model::UpdateWorkspaceIntegration {
        credentials: Some(request.credentials),
        ..Default::default()
    };

    let integration = conn
        .update_workspace_integration(path_params.integration_id, changes)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Integration credentials updated");

    Ok((StatusCode::OK, Json(Integration::from_model(integration))))
}

fn update_integration_credentials_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update integration credentials")
        .description("Updates only the authentication credentials for an integration.")
        .response::<200, Json<Integration>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace integration.
///
/// Permanently removes the integration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn delete_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace integration");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the integration first to get workspace context for authorization
    let integration = find_integration(&mut conn, path_params.integration_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            integration.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    conn.delete_workspace_integration(path_params.integration_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Integration deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_integration_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete integration")
        .description("Permanently removes the integration from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds an integration by ID or returns NotFound error.
async fn find_integration(
    conn: &mut nvisy_postgres::PgConn,
    integration_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceIntegration> {
    conn.find_workspace_integration_by_id(integration_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Integration not found")
                .with_resource("integration")
        })
}

/// Returns routes for workspace integration management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceId}/integrations/",
            post_with(create_integration, create_integration_docs)
                .get_with(list_integrations, list_integrations_docs),
        )
        // Integration-specific routes (integration ID is globally unique)
        .api_route(
            "/integrations/{integrationId}/",
            get_with(read_integration, read_integration_docs)
                .put_with(update_integration, update_integration_docs)
                .delete_with(delete_integration, delete_integration_docs),
        )
        .api_route(
            "/integrations/{integrationId}/credentials/",
            patch_with(
                update_integration_credentials,
                update_integration_credentials_docs,
            ),
        )
        .with_path_items(|item| item.tag("Integrations"))
}
