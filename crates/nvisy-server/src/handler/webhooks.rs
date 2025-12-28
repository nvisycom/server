//! Project webhook management handlers.
//!
//! This module provides comprehensive project webhook management functionality,
//! allowing project administrators to create, configure, and manage webhooks
//! for receiving event notifications. All operations are secured with proper
//! authorization and follow role-based access control principles.

use aide::axum::ApiRouter;
use axum::http::StatusCode;
use nvisy_postgres::query::{Pagination, ProjectWebhookRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{
    CreateWebhook, ProjectPathParams, UpdateWebhook as UpdateWebhookRequest, WebhookPathParams,
};
use crate::handler::response::{Webhook, WebhookWithSecret, Webhooks};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project webhook operations.
const TRACING_TARGET: &str = "nvisy_server::handler::webhooks";

/// Creates a new project webhook.
///
/// Returns the webhook with secret. The secret is only shown once at creation.
/// Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn create_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookWithSecret>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating project webhook");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let new_webhook = request.into_model(path_params.project_id, auth_state.account_id);
    let webhook = conn.create_project_webhook(new_webhook).await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = %webhook.id,
        "Webhook created ",
    );

    Ok((StatusCode::CREATED, Json(webhook.into())))
}

/// Lists all webhooks for a project.
///
/// Returns all configured webhooks without secrets. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn list_webhooks(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<Webhooks>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project webhooks");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let webhooks = conn
        .list_project_webhooks(path_params.project_id, Pagination::default())
        .await?;

    let webhooks: Webhooks = webhooks.into_iter().map(Into::into).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        webhook_count = webhooks.len(),
        "Project webhooks listed ",
    );

    Ok((StatusCode::OK, Json(webhooks)))
}

/// Retrieves a specific project webhook.
///
/// Returns webhook details without secret. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn read_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading project webhook");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let webhook = find_project_webhook(&mut conn, &path_params).await?;

    tracing::debug!(target: TRACING_TARGET, "Project webhook read");

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Updates a project webhook.
///
/// Updates webhook configuration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn update_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(request): ValidateJson<UpdateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating project webhook");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let _ = find_project_webhook(&mut conn, &path_params).await?;

    let update_data = request.into_model();
    let webhook = conn
        .update_project_webhook(path_params.webhook_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Webhook updated");

    Ok((StatusCode::OK, Json(webhook.into())))
}

/// Deletes a project webhook.
///
/// Permanently removes the webhook. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn delete_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting project webhook");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Verify webhook exists and belongs to the project
    let _ = find_project_webhook(&mut conn, &path_params).await?;

    conn.delete_project_webhook(path_params.webhook_id).await?;

    tracing::info!(target: TRACING_TARGET, "Webhook deleted");

    Ok(StatusCode::NO_CONTENT)
}

/// Finds a webhook by ID and verifies it belongs to the specified project.
async fn find_project_webhook(
    conn: &mut nvisy_postgres::PgConn,
    path_params: &WebhookPathParams,
) -> Result<nvisy_postgres::model::ProjectWebhook> {
    let Some(webhook) = conn
        .find_project_webhook_by_id(path_params.webhook_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Webhook not found")
            .with_resource("webhook"));
    };

    if webhook.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_message("Webhook not found")
            .with_resource("webhook"));
    }

    Ok(webhook)
}

/// Returns routes for project webhook management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/{project_id}/webhooks/", post(create_webhook))
        .api_route("/projects/{project_id}/webhooks/", get(list_webhooks))
        .api_route(
            "/projects/{project_id}/webhooks/{webhook_id}/",
            get(read_webhook),
        )
        .api_route(
            "/projects/{project_id}/webhooks/{webhook_id}/",
            put(update_webhook),
        )
        .api_route(
            "/projects/{project_id}/webhooks/{webhook_id}/",
            delete(delete_webhook),
        )
        .with_path_items(|item| item.tag("Webhooks"))
}
