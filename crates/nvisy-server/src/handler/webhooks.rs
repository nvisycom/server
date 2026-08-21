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
use nvisy_postgres::types::{WebhookId, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use nvisy_webhook::WebhookService;
use nvisy_webhook::guard::UrlGuardExt;
use nvisy_webhook::provider::{WebhookContext, WebhookRequest};
use url::Url;
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{
    CreateWebhook, CursorPagination, TestWebhook, UpdateWebhook as UpdateWebhookRequest,
    WebhookPathParams,
};
use crate::handler::response::{
    ErrorResponse, Webhook, WebhookCreated, WebhookResult, WebhooksPage,
};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{
    CryptoService, EventEmitter, EventOrigin, ServiceState, WebhookRef, WorkspaceEvent,
};

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
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWebhook>,
) -> Result<(StatusCode, Json<WebhookCreated>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::CreateWebhooks)
        .await?;

    check_webhook_url(&request.url)?;

    // Generate the signing secret here so it is returned once and stored only
    // encrypted; the server decrypts it to sign each delivery.
    let secret = crypto.generate_secret();
    let encrypted_secret = crypto.encrypt(workspace.id, secret.as_bytes())?;

    let new_webhook = request.into_model(workspace.id, auth_state.account_id, encrypted_secret)?;

    // Create the webhook and record the outbox event atomically, so the event is
    // never lost, nor recorded for a create that rolled back.
    let webhook = conn
        .transaction(async |conn| {
            let webhook = conn.create_workspace_webhook(new_webhook).await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id: auth_state.account_id,
                    security: &security,
                },
                WorkspaceEvent::WebhookCreated(WebhookRef {
                    webhook_id: webhook.id,
                    webhook_name: webhook.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(webhook)
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        webhook_id = %WebhookId::from_uuid(webhook.id),
        "Webhook created",
    );

    // The creator is the authenticated caller; resolve their handle directly.
    let creator = resolve_account_ref(&mut conn, auth_state.account_id).await?;

    // WebhookCreated includes the secret, which is visible only once.
    Ok((
        StatusCode::CREATED,
        Json(WebhookCreated::from_model(
            webhook,
            workspace.slug,
            creator,
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
        Json(WebhooksPage::from_cursor_page(page, |wc| {
            Webhook::from_model(wc.item, workspace.slug.clone(), wc.account.into())
        })),
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

    let found = find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace webhook read");

    Ok((
        StatusCode::OK,
        Json(Webhook::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
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
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::UpdateWebhooks)
        .await?;

    let existing = find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid())
        .await?
        .item;

    if let Some(url) = &request.url {
        check_webhook_url(url)?;
    }

    let update_data = request.into_model(existing.status)?;
    // The effective post-update name: the new one if the request set it, else the
    // existing name.
    let webhook_name = update_data
        .display_name
        .clone()
        .unwrap_or_else(|| existing.display_name.clone());

    // Update the webhook and record the outbox event atomically, so the event is
    // never lost, nor recorded for an update that rolled back.
    conn.transaction(async |conn| {
        conn.update_workspace_webhook(existing.id, update_data)
            .await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::WebhookUpdated(WebhookRef {
                webhook_id: existing.id,
                webhook_name,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let found = find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid()).await?;

    tracing::info!(target: TRACING_TARGET, "Webhook updated");

    Ok((
        StatusCode::OK,
        Json(Webhook::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
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
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::DeleteWebhooks)
        .await?;

    let existing = find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid())
        .await?
        .item;

    // Delete the webhook and record the outbox event atomically, so the event is
    // never lost, nor recorded for a delete that rolled back.
    conn.transaction(async |conn| {
        conn.delete_workspace_webhook(existing.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: auth_state.account_id,
                security: &security,
            },
            WorkspaceEvent::WebhookDeleted(WebhookRef {
                webhook_id: existing.id,
                webhook_name: existing.display_name.clone(),
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
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
    State(crypto): State<CryptoService>,
    State(webhook_service): State<WebhookService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<WebhookPathParams>,
    ValidateJson(request): ValidateJson<TestWebhook>,
) -> Result<(StatusCode, Json<WebhookResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Testing workspace webhook");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::TestWebhooks)
        .await?;

    let webhook = find_webhook(&mut conn, workspace.id, path_params.webhook_id.as_uuid())
        .await?
        .item;

    // Parse the webhook URL
    let url: Url = webhook.url.parse().map_err(|_| {
        ErrorKind::BadRequest
            .with_message("Invalid webhook URL")
            .with_resource("webhook")
    })?;

    // Build a signed test request that mirrors a real delivery: decrypt the
    // secret so the signature is present, carry the webhook's custom headers,
    // and include the caller's payload so they can exercise their own body.
    let secret = String::from_utf8(crypto.decrypt(workspace.id, &webhook.encrypted_secret)?)
        .map_err(|_| {
            ErrorKind::InternalServerError.with_message("webhook secret is not valid UTF-8")
        })?;

    let mut context =
        WebhookContext::test(webhook.id, webhook.workspace_id).with_account(auth_state.account_id);
    if let Some(payload) = request.payload {
        context = context.with_metadata(payload);
    }

    let mut webhook_request = WebhookRequest::new(
        url,
        "webhook:test",
        "This is a test webhook delivery",
        context,
    )
    .with_secret(secret);
    let headers = webhook.parsed_headers();
    if !headers.is_empty() {
        webhook_request = webhook_request.with_headers(headers.into_map().into_iter().collect());
    }

    let response = webhook_service.deliver(&webhook_request).await?;

    // A manual test does not touch stored delivery health: its failures must not
    // count toward the worker's auto-disable threshold, and its successes must
    // not mask a genuinely failing endpoint. The result is returned to the
    // caller directly.
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

/// Validates a webhook URL at write time: `http`/`https` scheme and, for a
/// literal-IP host, a globally routable address.
///
/// This is fast feedback; the delivery worker additionally rejects hostnames
/// that resolve to non-routable addresses (which cannot be checked here without
/// DNS).
fn check_webhook_url(url: &str) -> Result<()> {
    let parsed: Url = url
        .parse()
        .map_err(|_| ErrorKind::BadRequest.with_message("invalid webhook URL"))?;
    parsed
        .check_scheme()
        .map_err(|_| ErrorKind::BadRequest.with_message("webhook URL must use http or https"))?;
    parsed.check_literal_host().map_err(|_| {
        ErrorKind::BadRequest.with_message("webhook URL must not target an internal address")
    })
}

/// Finds a webhook within a workspace by id, with its creator, or returns a
/// NotFound error.
async fn find_webhook(
    conn: &mut PgConn,
    workspace_id: Uuid,
    webhook_id: Uuid,
) -> Result<WithAccountRef<WorkspaceWebhook>> {
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
                .patch_with(update_webhook, update_webhook_docs)
                .delete_with(delete_webhook, delete_webhook_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/webhooks/{webhookId}/test/",
            post_with(test_webhook, test_webhook_docs),
        )
        .with_path_items(|item| item.tag("Webhooks"))
}
