//! Project webhook management handlers.
//!
//! This module provides comprehensive project webhook management functionality,
//! allowing project administrators to create, configure, and manage webhooks
//! for receiving event notifications. All operations are secured with proper
//! authorization and follow role-based access control principles.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewProjectWebhook, UpdateProjectWebhook};
use nvisy_postgres::query::{Pagination, ProjectWebhookRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::request::{
    CreateWebhook, ProjectPathParams, UpdateWebhook as UpdateProjectWebhookRequest,
    UpdateWebhookStatus, WebhookPathParams,
};
use crate::handler::response::{Webhook, WebhookWithSecret, Webhooks};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project webhook operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_webhook";

/// Creates a new project webhook.
///
/// Returns the webhook with secret. The secret is only shown once at creation.
#[tracing::instrument(skip_all)]
async fn create_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(payload): ValidateJson<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookWithSecret>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        display_name = payload.display_name,
        "Creating project webhook"
    );

    // Verify user has permission to manage webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Convert events to Vec<Option<String>> as expected by the model
    let events: Vec<Option<String>> = payload.events.into_iter().map(Some).collect();

    // Create the webhook
    let new_webhook = NewProjectWebhook {
        project_id: path_params.project_id,
        display_name: payload.display_name,
        description: payload.description,
        url: payload.url,
        secret: payload.secret,
        events,
        headers: payload.headers,
        status: None,
        max_failures: payload.max_failures,
        created_by: auth_claims.account_id,
    };

    let webhook = pg_client.create_project_webhook(new_webhook).await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = webhook.id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Webhook created successfully"
    );

    Ok((StatusCode::CREATED, Json(webhook.into())))
}

/// Lists all webhooks for a project.
#[tracing::instrument(skip_all)]
async fn list_webhooks(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<Webhooks>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Listing project webhooks"
    );

    // Verify user has permission to view webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let webhooks = pg_client
        .list_project_webhooks(path_params.project_id, Pagination::default())
        .await?;

    let webhooks: Webhooks = webhooks.into_iter().map(Into::into).collect();

    Ok((StatusCode::OK, Json(webhooks)))
}

/// Retrieves a specific project webhook.
#[tracing::instrument(skip_all)]
async fn read_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        webhook_id = path_params.webhook_id.to_string(),
        "Reading project webhook"
    );

    // Verify user has permission to view webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let Some(webhook) = pg_client
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    };

    // Verify the webhook belongs to the specified project
    if webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    }

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Updates a project webhook.
#[tracing::instrument(skip_all)]
async fn update_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(payload): ValidateJson<UpdateProjectWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        webhook_id = path_params.webhook_id.to_string(),
        "Updating project webhook"
    );

    // Verify user has permission to manage webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let Some(existing_webhook) = pg_client
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    };

    if existing_webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    }

    // Convert events to Vec<Option<String>> if provided
    let events = payload.events.map(|e| e.into_iter().map(Some).collect());

    // Update the webhook
    let update_data = UpdateProjectWebhook {
        display_name: payload.display_name,
        description: payload.description,
        url: payload.url,
        secret: payload.secret.map(Some),
        events,
        headers: payload.headers,
        status: None,
        max_failures: payload.max_failures,
        ..Default::default()
    };

    let webhook = pg_client
        .update_project_webhook(path_params.webhook_id, update_data)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = path_params.webhook_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Webhook updated successfully"
    );

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Updates webhook status.
#[tracing::instrument(skip_all)]
async fn update_webhook_status(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(payload): ValidateJson<UpdateWebhookStatus>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        webhook_id = path_params.webhook_id.to_string(),
        "Updating webhook status"
    );

    // Verify user has permission to manage webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let Some(existing_webhook) = pg_client
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    };

    if existing_webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    }

    let update_data = UpdateProjectWebhook {
        status: Some(payload.status),
        ..Default::default()
    };

    let webhook = pg_client
        .update_project_webhook(path_params.webhook_id, update_data)
        .await?;

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Resets webhook failure count and reactivates if disabled.
#[tracing::instrument(skip_all)]
async fn reset_webhook_failures(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        webhook_id = path_params.webhook_id.to_string(),
        "Resetting webhook failures"
    );

    // Verify user has permission to manage webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let Some(existing_webhook) = pg_client
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    };

    if existing_webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    }

    let webhook = pg_client
        .reset_webhook_failures(path_params.webhook_id)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = path_params.webhook_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Webhook failures reset successfully"
    );

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Deletes a project webhook.
#[tracing::instrument(skip_all)]
async fn delete_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        webhook_id = path_params.webhook_id.to_string(),
        "Deleting project webhook"
    );

    // Verify user has permission to manage webhooks
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let Some(existing_webhook) = pg_client
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    };

    if existing_webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message(format!("Webhook not found: {}", path_params.webhook_id))
            .with_resource("webhook"));
    }

    pg_client
        .delete_project_webhook(path_params.webhook_id)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = path_params.webhook_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Webhook deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Returns routes for project webhook management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/:project_id/webhooks/", post(create_webhook))
        .api_route("/projects/:project_id/webhooks/", get(list_webhooks))
        .api_route(
            "/projects/:project_id/webhooks/:webhook_id/",
            get(read_webhook),
        )
        .api_route(
            "/projects/:project_id/webhooks/:webhook_id/",
            put(update_webhook),
        )
        .api_route(
            "/projects/:project_id/webhooks/:webhook_id/status/",
            patch(update_webhook_status),
        )
        .api_route(
            "/projects/:project_id/webhooks/:webhook_id/reset/",
            post(reset_webhook_failures),
        )
        .api_route(
            "/projects/:project_id/webhooks/:webhook_id/",
            delete(delete_webhook),
        )
}
