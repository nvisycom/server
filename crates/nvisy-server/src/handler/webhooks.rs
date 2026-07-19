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
use nvisy_postgres::model::WorkspaceWebhook;
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_postgres::types::Username;
use nvisy_postgres::{PgClient, PgConn};
use nvisy_webhook::WebhookService;
use nvisy_webhook::provider::WebhookRequest;
use url::Url;
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    CreateWebhook, CursorPagination, TestWebhook, UpdateWebhook as UpdateWebhookRequest,
    WebhookPathParams,
};
use crate::handler::response::{
    ErrorResponse, Webhook, WebhookCreated, WebhookResult, WebhooksPage,
};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{CryptoService, ServiceState};

/// Tracing target for workspace webhook operations.
const TRACING_TARGET: &str = "nvisy_server::handler::webhooks";

/// Creates a new workspace webhook.
///
/// Returns the webhook configuration. Requires `CreateWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn create_webhook(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookCreated>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::CreateWebhooks)
        .await?;

    // Generate the signing secret here so it is returned once and stored only
    // encrypted; the server decrypts it to sign each delivery.
    let secret = crypto.generate_secret();
    let encrypted_secret = crypto.encrypt(workspace.id, secret.as_bytes())?;

    let new_webhook = request.into_model(workspace.id, auth_state.account_id, encrypted_secret);
    let webhook = conn.create_workspace_webhook(new_webhook).await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = %webhook.id,
        "Webhook created",
    );

    let (webhook, creator_username) = find_webhook(&mut conn, workspace.id, webhook.id).await?;

    // Return WebhookCreated which includes the secret (visible only once)
    Ok((
        StatusCode::CREATED,
        Json(WebhookCreated::from_model(
            webhook,
            workspace.slug,
            creator_username,
            secret,
        )),
    ))
}

fn create_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create webhook")
        .description(
            "Creates a new webhook for the workspace. The response includes the signing secret \
             which is used for HMAC-SHA256 verification of webhook payloads. **Important**: The \
             secret is only shown once upon creation and cannot be retrieved again.",
        )
        .response::<201, Json<WebhookCreated>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all webhooks for a workspace.
///
/// Returns all configured webhooks. Requires `ViewWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_webhooks(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WebhooksPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace webhooks");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWebhooks)
        .await?;

    let page = conn
        .cursor_list_workspace_webhooks(workspace.id, pagination.into())
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        webhook_count = page.items.len(),
        "Workspace webhooks listed",
    );

    Ok((
        StatusCode::OK,
        Json(WebhooksPage::from_cursor_page(
            page,
            |(webhook, creator_username)| {
                Webhook::from_model(webhook, workspace.slug.clone(), creator_username)
            },
        )),
    ))
}

fn list_webhooks_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List webhooks")
        .description("Returns all configured webhooks for the workspace without secrets.")
        .response::<200, Json<WebhooksPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace webhook.
///
/// Returns webhook details. Requires `ViewWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn read_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewWebhooks)
        .await?;

    let (webhook, creator_username) =
        find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace webhook read");

    Ok((
        StatusCode::OK,
        Json(Webhook::from_model(
            webhook,
            workspace.slug,
            creator_username,
        )),
    ))
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
/// Updates webhook configuration. Requires `UpdateWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn update_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(request): ValidateJson<UpdateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdateWebhooks)
        .await?;

    let (existing, _) =
        find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    let update_data = request.into_model(existing.status);
    conn.update_workspace_webhook(existing.id, update_data)
        .await?;

    let (webhook, creator_username) =
        find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    tracing::info!(target: TRACING_TARGET, "Webhook updated");

    Ok((
        StatusCode::OK,
        Json(Webhook::from_model(
            webhook,
            workspace.slug,
            creator_username,
        )),
    ))
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
/// Permanently removes the webhook. Requires `DeleteWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn delete_webhook(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WebhookPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::DeleteWebhooks)
        .await?;

    let (existing, _) =
        find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    conn.delete_workspace_webhook(existing.id).await?;

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
/// Requires `TestWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn test_webhook(
    State(pg_client): State<PgClient>,
    State(webhook_service): State<WebhookService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(_request): ValidateJson<TestWebhook>,
) -> Result<(StatusCode, Json<WebhookResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Testing workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::TestWebhooks)
        .await?;

    let (webhook, _) =
        find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    // Parse the webhook URL
    let url: Url = webhook.url.parse().map_err(|_| {
        ErrorKind::BadRequest
            .with_message("Invalid webhook URL")
            .with_resource("webhook")
    })?;

    // Build the test webhook request
    let webhook_request = WebhookRequest::test(url, webhook.id, webhook.workspace_id);
    let response = webhook_service.deliver(&webhook_request).await?;

    // Update last_triggered_at timestamp
    if response.is_success() {
        conn.record_webhook_success(webhook.id).await?;
    } else {
        conn.record_webhook_failure(webhook.id).await?;
    }

    tracing::info!(
        target: TRACING_TARGET,
        success = response.is_success(),
        status_code = ?response.status_code,
        "Webhook test completed"
    );

    Ok((StatusCode::OK, Json(WebhookResult::from_response(response))))
}

fn test_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Test webhook")
        .description("Sends a test payload to the webhook endpoint and returns the result.")
        .response::<200, Json<WebhookResult>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Finds a webhook within a workspace by id, with its creator's handle, or
/// returns a NotFound error.
async fn find_webhook(
    conn: &mut PgConn,
    workspace_id: Uuid,
    webhook_id: Uuid,
) -> Result<(WorkspaceWebhook, Username)> {
    conn.find_webhook_in_workspace_with_creator(workspace_id, webhook_id)
        .await?
        .ok_or_else(|| Error::not_found("webhook"))
}

/// Returns routes for workspace webhook management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/webhooks/",
            post_with(create_webhook, create_webhook_docs)
                .get_with(list_webhooks, list_webhooks_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/webhooks/{webhookId}/",
            get_with(read_webhook, read_webhook_docs)
                .put_with(update_webhook, update_webhook_docs)
                .delete_with(delete_webhook, delete_webhook_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/webhooks/{webhookId}/test/",
            post_with(test_webhook, test_webhook_docs),
        )
        .with_path_items(|item| item.tag("Webhooks"))
}
