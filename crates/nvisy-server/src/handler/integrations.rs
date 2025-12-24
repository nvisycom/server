//! Project integration management handlers.
//!
//! This module provides comprehensive project integration management functionality,
//! allowing project administrators to create, configure, and manage integrations
//! with external services. All operations are secured with proper authorization
//! and follow role-based access control principles.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewProjectIntegration, UpdateProjectIntegration};
use nvisy_postgres::query::ProjectIntegrationRepository;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::request::{
    CreateProjectIntegration, UpdateIntegrationCredentials, UpdateIntegrationMetadata,
    UpdateIntegrationStatus, UpdateProjectIntegration as UpdateProjectIntegrationRequest,
};
use crate::handler::response::{
    ProjectIntegration, ProjectIntegrationSummaries, ProjectIntegrationWithCredentials,
};
use crate::handler::{ErrorKind, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for project integration operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_integration";

/// Combined path parameters for integration-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the integration.
    pub integration_id: Uuid,
}

/// Creates a new project integration.
#[tracing::instrument(skip_all)]
async fn create_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(payload): ValidateJson<CreateProjectIntegration>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_name = payload.integration_name,
        "Creating project integration"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Check if integration name is already used in this project
    let name_exists = pg_client
        .is_integration_name_unique(path_params.project_id, &payload.integration_name, None)
        .await?;

    if !name_exists {
        return Err(ErrorKind::Conflict.with_message("Integration name already exists in project"));
    }

    // Create the integration
    let new_integration = NewProjectIntegration {
        project_id: path_params.project_id,
        integration_name: payload.integration_name,
        description: payload.description,
        integration_type: payload.integration_type,
        metadata: payload.metadata,
        credentials: payload.credentials,
        is_active: payload.is_active,
        last_sync_at: None,
        sync_status: None,
        created_by: auth_claims.account_id,
    };

    let integration = pg_client.create_integration(new_integration).await?;

    tracing::info!(
        target: TRACING_TARGET,
        integration_id = integration.id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Integration created successfully"
    );

    Ok((StatusCode::CREATED, Json(integration.into())))
}

/// Lists all integrations for a project.
#[tracing::instrument(skip_all)]
async fn list_integrations(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(_pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ProjectIntegrationSummaries>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Listing project integrations"
    );

    // Verify user has permission to view integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let integrations = pg_client
        .list_project_integrations(path_params.project_id)
        .await?;

    let integration_summaries: ProjectIntegrationSummaries = integrations
        .into_iter()
        .map(|integration| integration.into())
        .collect();

    Ok((StatusCode::OK, Json(integration_summaries)))
}

/// Retrieves a specific project integration.
#[tracing::instrument(skip_all)]
async fn read_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Reading project integration"
    );

    // Verify user has permission to view integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let Some(integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    // Verify the integration belongs to the specified project
    if integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Retrieves a project integration with credentials.
#[tracing::instrument(skip_all)]
async fn read_integration_with_credentials(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<(StatusCode, Json<ProjectIntegrationWithCredentials>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Reading project integration with credentials"
    );

    // Verify user has permission to manage integrations (higher permission for credentials)
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let Some(integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    // Verify the integration belongs to the specified project
    if integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates a project integration.
#[tracing::instrument(skip_all)]
async fn update_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(payload): ValidateJson<UpdateProjectIntegrationRequest>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Updating project integration"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let Some(existing_integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    if existing_integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    // Check if new name conflicts with existing integrations (if name is being changed)
    if let Some(ref new_name) = payload.integration_name
        && new_name != &existing_integration.integration_name
    {
        let name_exists = pg_client
            .is_integration_name_unique(
                path_params.project_id,
                new_name,
                Some(path_params.integration_id),
            )
            .await?;

        if !name_exists {
            return Err(
                ErrorKind::Conflict.with_message("Integration name already exists in project")
            );
        }
    }

    // Update the integration
    let update_data = UpdateProjectIntegration {
        integration_name: payload.integration_name,
        description: payload.description,
        integration_type: payload.integration_type,
        metadata: payload.metadata,
        credentials: payload.credentials,
        is_active: payload.is_active,
        last_sync_at: None,
        sync_status: None,
    };

    let integration = pg_client
        .update_integration(path_params.integration_id, update_data)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        integration_id = path_params.integration_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Integration updated successfully"
    );

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates integration status.
#[tracing::instrument(skip_all)]
async fn update_integration_status(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(payload): ValidateJson<UpdateIntegrationStatus>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Updating integration status"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let Some(existing_integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    if existing_integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    let integration = pg_client
        .update_integration_status(
            path_params.integration_id,
            payload.sync_status,
            auth_claims.account_id,
        )
        .await?;

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates integration credentials.
#[tracing::instrument(skip_all)]
async fn update_integration_credentials(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(payload): ValidateJson<UpdateIntegrationCredentials>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Updating integration credentials"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let Some(existing_integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    if existing_integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    let integration = pg_client
        .update_integration_auth(
            path_params.integration_id,
            payload.credentials,
            auth_claims.account_id,
        )
        .await?;

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Updates integration metadata.
#[tracing::instrument(skip_all)]
async fn update_integration_metadata(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
    ValidateJson(payload): ValidateJson<UpdateIntegrationMetadata>,
) -> Result<(StatusCode, Json<ProjectIntegration>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Updating integration metadata"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let Some(existing_integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    if existing_integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    let integration = pg_client
        .update_integration_metadata(
            path_params.integration_id,
            payload.metadata,
            auth_claims.account_id,
        )
        .await?;

    Ok((StatusCode::OK, Json(integration.into())))
}

/// Deletes a project integration.
#[tracing::instrument(skip_all)]
async fn delete_integration(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<IntegrationPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        integration_id = path_params.integration_id.to_string(),
        "Deleting project integration"
    );

    // Verify user has permission to manage integrations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify integration exists and belongs to the project
    let Some(existing_integration) = pg_client
        .find_integration_by_id(path_params.integration_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    };

    if existing_integration.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!(
                "Integration not found: {}",
                path_params.integration_id
            ))
            .with_resource("integration"));
    }

    pg_client
        .delete_integration(path_params.integration_id)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        integration_id = path_params.integration_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Integration deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Returns routes for project integration management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/:project_id/integrations/",
            post(create_integration),
        )
        .api_route(
            "/projects/:project_id/integrations/",
            get(list_integrations),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/",
            get(read_integration),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/credentials/",
            get(read_integration_with_credentials),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/",
            put(update_integration),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/status/",
            patch(update_integration_status),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/credentials/",
            patch(update_integration_credentials),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/metadata/",
            patch(update_integration_metadata),
        )
        .api_route(
            "/projects/:project_id/integrations/:integration_id/",
            delete(delete_integration),
        )
}

#[cfg(test)]
mod test {
    use crate::handler::test::create_test_server;

    #[tokio::test]
    async fn test_create_integration_success() -> anyhow::Result<()> {
        let _server = create_test_server().await?;

        // TODO: Add test implementation
        // This would require creating a test project first and authenticating

        Ok(())
    }

    #[tokio::test]
    async fn test_list_integrations() -> anyhow::Result<()> {
        let _server = create_test_server().await?;

        // TODO: Add test implementation

        Ok(())
    }

    #[tokio::test]
    async fn test_update_integration() -> anyhow::Result<()> {
        let _server = create_test_server().await?;

        // TODO: Add test implementation

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_integration() -> anyhow::Result<()> {
        let _server = create_test_server().await?;

        // TODO: Add test implementation

        Ok(())
    }
}
