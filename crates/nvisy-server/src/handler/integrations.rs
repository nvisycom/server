//! Project integration management handlers.
//!
//! This module provides comprehensive project integration management functionality,
//! allowing project administrators to create, configure, and manage integrations
//! with external services. All operations are secured with proper authorization
//! and follow role-based access control principles.

use aide::axum::ApiRouter;
use axum::http::StatusCode;
use nvisy_postgres::query::ProjectIntegrationRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{
    CreateProjectIntegration, IntegrationPathParams, Pagination, ProjectPathParams,
    UpdateIntegrationCredentials, UpdateProjectIntegration,
};
use crate::handler::response::{Integration, Integrations};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project integration operations.
const TRACING_TARGET: &str = "nvisy_server::handler::integrations";

/// Creates a new project integration.
///
/// Creates an integration with an external service. Requires `ManageIntegrations`
/// permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        integration_type = ?request.integration_type,
    )
)]
async fn create_integration(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateProjectIntegration>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::info!(target: TRACING_TARGET, "Creating project integration");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Check if integration name is already used in this project
    let name_is_unique = conn
        .is_integration_name_unique(path_params.project_id, &request.integration_name, None)
        .await?;

    if !name_is_unique {
        return Err(ErrorKind::Conflict.with_message("Integration name already exists in project"));
    }

    let new_integration = request.into_model(path_params.project_id, auth_state.account_id);
    let integration = conn.create_integration(new_integration).await?;

    tracing::info!(
        target: TRACING_TARGET,
        integration_id = %integration.id,
        "Integration created successfully",
    );

    Ok((StatusCode::CREATED, Json(integration.into())))
}

/// Lists all integrations for a project.
///
/// Returns all configured integrations. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn list_integrations(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(_pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Integrations>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project integrations");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let integrations = conn
        .list_project_integrations(path_params.project_id)
        .await?;

    let integrations: Integrations = integrations.into_iter().map(Into::into).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        integration_count = integrations.len(),
        "Project integrations listed successfully",
    );

    Ok((StatusCode::OK, Json(integrations)))
}

/// Retrieves a specific project integration.
///
/// Returns integration details. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn read_integration(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading project integration");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let integration = find_project_integration(&mut conn, &path_params).await?;

    tracing::debug!(target: TRACING_TARGET, "Project integration retrieved successfully");

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates a project integration.
///
/// Updates integration configuration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn update_integration(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(request): ValidateJson<UpdateProjectIntegration>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::info!(target: TRACING_TARGET, "Updating project integration");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let existing = find_project_integration(&mut conn, &path_params).await?;

    // Check if new name conflicts with existing integrations
    if let Some(ref new_name) = request.integration_name
        && new_name != &existing.integration_name
    {
        let name_is_unique = conn
            .is_integration_name_unique(
                path_params.project_id,
                new_name,
                Some(path_params.integration_id),
            )
            .await?;

        if !name_is_unique {
            return Err(
                ErrorKind::Conflict.with_message("Integration name already exists in project")
            );
        }
    }

    let integration = conn
        .update_integration(path_params.integration_id, request.into_model())
        .await?;

    tracing::info!(target: TRACING_TARGET, "Integration updated successfully");
    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates integration credentials.
///
/// Updates only the authentication credentials. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn update_integration_credentials(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(request): ValidateJson<UpdateIntegrationCredentials>,
) -> Result<(StatusCode, Json<Integration>)> {
    tracing::info!(target: TRACING_TARGET, "Updating integration credentials");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let _ = find_project_integration(&mut conn, &path_params).await?;

    let integration = conn
        .update_integration_auth(
            path_params.integration_id,
            request.credentials,
            auth_state.account_id,
        )
        .await?;

    tracing::info!(target: TRACING_TARGET, "Integration credentials updated successfully");

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Deletes a project integration.
///
/// Permanently removes the integration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        integration_id = %path_params.integration_id,
    )
)]
async fn delete_integration(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Deleting project integration");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let _ = find_project_integration(&mut conn, &path_params).await?;

    conn.delete_integration(path_params.integration_id).await?;

    tracing::warn!(target: TRACING_TARGET, "Integration deleted successfully");

    Ok(StatusCode::NO_CONTENT)
}

/// Finds an integration by ID and verifies it belongs to the specified project.
async fn find_project_integration(
    conn: &mut nvisy_postgres::PgConn,
    path_params: &IntegrationPathParams,
) -> Result<nvisy_postgres::model::ProjectIntegration> {
    let Some(integration) = conn
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Integration not found")
            .with_resource("integration"));
    };

    if integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message("Integration not found")
            .with_resource("integration"));
    }

    Ok(integration)
}

/// Returns routes for project integration management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/{project_id}/integrations/",
            post(create_integration),
        )
        .api_route(
            "/projects/{project_id}/integrations/",
            get(list_integrations),
        )
        .api_route(
            "/projects/{project_id}/integrations/{integration_id}/",
            get(read_integration),
        )
        .api_route(
            "/projects/{project_id}/integrations/{integration_id}/",
            put(update_integration),
        )
        .api_route(
            "/projects/{project_id}/integrations/{integration_id}/credentials/",
            patch(update_integration_credentials),
        )
        .api_route(
            "/projects/{project_id}/integrations/{integration_id}/",
            delete(delete_integration),
        )
}
