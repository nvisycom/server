//! Workspace webhook management handlers.
//!
//! This module provides comprehensive workspace webhook management functionality,
//! allowing workspace administrators to create, configure, and manage webhooks
//! for receiving event notifications. All operations are secured with proper
//! authorization and follow role-based access control principles.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::query::{Pagination, WorkspaceWebhookRepository};
use nvisy_service::webhook::{WebhookRequest, WebhookService};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{
    CreateWebhook, TestWebhook, UpdateWebhook as UpdateWebhookRequest, WebhookPathParams,
    WorkspacePathParams,
};
use crate::handler::response::{
    ErrorResponse, Webhook, WebhookTestResult, WebhookWithSecret, Webhooks,
};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace webhook operations.
const TRACING_TARGET: &str = "nvisy_server::handler::webhooks";

/// Creates a new workspace webhook.
///
/// Returns the webhook with secret. The secret is only shown once at creation.
/// Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookWithSecret>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace webhook");

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let new_webhook = request.into_model(path_params.workspace_id, auth_state.account_id);
    let webhook = conn.create_workspace_webhook(new_webhook).await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = %webhook.id,
        "Webhook created ",
    );

    Ok((
        StatusCode::CREATED,
        Json(WebhookWithSecret::from_model(webhook)),
    ))
}

fn create_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create webhook")
        .description("Creates a new webhook. The secret is only shown once at creation.")
        .response::<201, Json<WebhookWithSecret>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all webhooks for a workspace.
///
/// Returns all configured webhooks without secrets. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_webhooks(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<(StatusCode, Json<Webhooks>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace webhooks");

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let webhooks = conn
        .list_workspace_webhooks(path_params.workspace_id, Pagination::default())
        .await?;

    let webhooks: Webhooks = Webhook::from_models(webhooks);

    tracing::debug!(
        target: TRACING_TARGET,
        webhook_count = webhooks.len(),
        "Workspace webhooks listed ",
    );

    Ok((StatusCode::OK, Json(webhooks)))
}

fn list_webhooks_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List webhooks")
        .description("Returns all configured webhooks for the workspace without secrets.")
        .response::<200, Json<Webhooks>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace webhook.
///
/// Returns webhook details without secret. Requires `ViewIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn read_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace webhook");

    // Fetch the webhook first to get workspace context for authorization
    let webhook = find_webhook(&mut conn, path_params.webhook_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            webhook.workspace_id,
            Permission::ViewIntegrations,
        )
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace webhook read");

    Ok((StatusCode::OK, Json(Webhook::from_model(webhook))))
}

fn read_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get webhook")
        .description("Returns webhook details without the secret.")
        .response::<200, Json<Webhook>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace webhook.
///
/// Updates webhook configuration. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn update_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(request): ValidateJson<UpdateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace webhook");

    // Fetch the webhook first to get workspace context for authorization
    let existing = find_webhook(&mut conn, path_params.webhook_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    let update_data = request.into_model();
    let webhook = conn
        .update_workspace_webhook(path_params.webhook_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Webhook updated");

    Ok((StatusCode::OK, Json(Webhook::from_model(webhook))))
}

fn update_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update webhook")
        .description("Updates webhook configuration such as URL or event subscriptions.")
        .response::<200, Json<Webhook>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace webhook.
///
/// Permanently removes the webhook. Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn delete_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace webhook");

    // Fetch the webhook first to get workspace context for authorization
    let webhook = find_webhook(&mut conn, path_params.webhook_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            webhook.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    conn.delete_workspace_webhook(path_params.webhook_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Webhook deleted");

    Ok(StatusCode::NO_CONTENT)
}

fn delete_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete webhook")
        .description("Permanently removes the webhook from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Tests a webhook by sending a test payload.
///
/// Sends a test request to the webhook endpoint and returns the result.
/// Requires `ManageIntegrations` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn test_webhook(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    State(webhook_service): State<WebhookService>,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(request): ValidateJson<TestWebhook>,
) -> Result<(StatusCode, Json<WebhookTestResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Testing workspace webhook");

    // Fetch the webhook to get URL and secret
    let webhook = find_webhook(&mut conn, path_params.webhook_id).await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            webhook.workspace_id,
            Permission::ManageIntegrations,
        )
        .await?;

    // Build the webhook request
    let payload = request.payload.unwrap_or_else(|| {
        serde_json::json!({
            "event": "test",
            "message": "This is a test webhook delivery"
        })
    });

    let mut webhook_request = WebhookRequest::new(&webhook.url, payload);
    if let Some(secret) = webhook.secret {
        webhook_request = webhook_request.with_secret(secret);
    }

    let response = webhook_service
        .deliver(&webhook_request)
        .await
        .map_err(|e| ErrorKind::InternalServerError.with_message(e.to_string()))?;

    tracing::info!(
        target: TRACING_TARGET,
        success = response.success,
        status_code = ?response.status_code,
        "Webhook test completed"
    );

    Ok((
        StatusCode::OK,
        Json(WebhookTestResult::from_core_response(response)),
    ))
}

fn test_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Test webhook")
        .description("Sends a test payload to the webhook endpoint and returns the result.")
        .response::<200, Json<WebhookTestResult>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a webhook by ID or returns NotFound error.
async fn find_webhook(
    conn: &mut nvisy_postgres::PgConn,
    webhook_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceWebhook> {
    conn.find_workspace_webhook_by_id(webhook_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Webhook not found")
                .with_resource("webhook")
        })
}

/// Returns routes for workspace webhook management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspace_id}/webhooks/",
            post_with(create_webhook, create_webhook_docs)
                .get_with(list_webhooks, list_webhooks_docs),
        )
        // Webhook-specific routes (webhook ID is globally unique)
        .api_route(
            "/webhooks/{webhook_id}/",
            get_with(read_webhook, read_webhook_docs)
                .put_with(update_webhook, update_webhook_docs)
                .delete_with(delete_webhook, delete_webhook_docs),
        )
        .api_route(
            "/webhooks/{webhook_id}/test/",
            post_with(test_webhook, test_webhook_docs),
        )
        .with_path_items(|item| item.tag("Webhooks"))
}
