//! Workspace webhook management handlers.
//!
//! Thin HTTP layer over [`WorkspaceWebhookService`]: authorize, extract the
//! request, call the service, and map the result to a response. The webhook
//! rules — secret minting, URL validation, lifecycle events, and test delivery —
//! live in the service.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceWebhook, CursorPagination, TestWorkspaceWebhook, UpdateWorkspaceWebhook,
    WorkspaceWebhookPathParams,
};
use crate::handler::response::{
    WorkspaceWebhook, WorkspaceWebhookCreated, WorkspaceWebhookResult, WorkspaceWebhooksPage,
};
use crate::handler::utility::resolve_account_ref;
use crate::response::{ErrorResponse, Result};
use crate::service::{ServiceState, event};

/// Tracing target for workspace webhook operations.
const TRACING_TARGET: &str = "nvisy_server::handler::webhooks";

/// Creates a new workspace webhook.
///
/// Returns the webhook configuration. Requires `CreateWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_webhook(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::CreateWebhooks>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceWebhook>,
) -> Result<(StatusCode, Json<WorkspaceWebhookCreated>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace webhook");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let created = webhooks
        .create(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            request.into(),
        )
        .await?;

    // The creator is the authenticated caller; resolve their handle directly.
    let mut conn = pg_client.get_connection().await?;
    let creator = resolve_account_ref(&mut conn, account_id).await?;

    // WebhookCreated includes the secret, which is visible only once.
    Ok((
        StatusCode::CREATED,
        Json(WorkspaceWebhookCreated::from_model(
            created.webhook,
            workspace.id,
            workspace.slug,
            creator,
            created.secret,
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
        .response::<201, Json<WorkspaceWebhookCreated>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_webhooks(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    authz: Authorized<markers::ViewWebhooks>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceWebhooksPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace webhooks");

    let workspace = authz.workspace;
    let page = webhooks
        .list(workspace.id, pagination.into_cursor())
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceWebhooksPage::from_cursor_page(page, |wc| {
            WorkspaceWebhook::from_model(
                wc.item,
                workspace.id,
                workspace.slug.clone(),
                wc.account.into(),
            )
        })),
    ))
}

fn list_webhooks_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List webhooks")
        .description("Returns all configured webhooks for the workspace without secrets.")
        .response::<200, Json<WorkspaceWebhooksPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace webhook.
///
/// Returns webhook details. Requires `ViewWebhooks` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn read_webhook(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    authz: Authorized<markers::ViewWebhooks>,
    Path(path_params): Path<WorkspaceWebhookPathParams>,
) -> Result<(StatusCode, Json<WorkspaceWebhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace webhook");

    let workspace = authz.workspace;
    let found = webhooks
        .find(workspace.id, path_params.webhook_id.as_uuid())
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceWebhook::from_model(
            found.item,
            workspace.id,
            workspace.slug,
            found.account.into(),
        )),
    ))
}

fn read_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get webhook")
        .description("Returns webhook details without the secret.")
        .response::<200, Json<WorkspaceWebhook>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn update_webhook(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    authz: Authorized<markers::UpdateWebhooks>,
    Path(path_params): Path<WorkspaceWebhookPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceWebhook>,
) -> Result<(StatusCode, Json<WorkspaceWebhook>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace webhook");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    let found = webhooks
        .update(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            path_params.webhook_id.as_uuid(),
            request.into(),
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceWebhook::from_model(
            found.item,
            workspace.id,
            workspace.slug,
            found.account.into(),
        )),
    ))
}

fn update_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update webhook")
        .description("Updates webhook configuration such as URL or event subscriptions.")
        .response::<200, Json<WorkspaceWebhook>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn delete_webhook(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    authz: Authorized<markers::DeleteWebhooks>,
    Path(path_params): Path<WorkspaceWebhookPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace webhook");

    let workspace = authz.workspace;
    let account_id = authz.account_id;
    webhooks
        .delete(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            path_params.webhook_id.as_uuid(),
        )
        .await?;

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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        webhook_id = %path_params.webhook_id,
    )
)]
async fn test_webhook(
    State(webhooks): State<domain::WorkspaceWebhookService>,
    authz: Authorized<markers::TestWebhooks>,
    Path(path_params): Path<WorkspaceWebhookPathParams>,
    ValidateJson(request): ValidateJson<TestWorkspaceWebhook>,
) -> Result<(StatusCode, Json<WorkspaceWebhookResult>)> {
    tracing::debug!(target: TRACING_TARGET, "Testing workspace webhook");

    let workspace = authz.workspace;
    let response = webhooks
        .test(
            workspace.id,
            authz.account_id,
            path_params.webhook_id.as_uuid(),
            request.payload,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceWebhookResult::from_response(response)),
    ))
}

fn test_webhook_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Test webhook")
        .description("Sends a test payload to the webhook endpoint and returns the result.")
        .response::<200, Json<WorkspaceWebhookResult>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for workspace webhook management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/webhooks/",
            post_with(create_webhook, create_webhook_docs)
                .get_with(list_webhooks, list_webhooks_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/webhooks/{webhookId}/",
            get_with(read_webhook, read_webhook_docs)
                .patch_with(update_webhook, update_webhook_docs)
                .delete_with(delete_webhook, delete_webhook_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/webhooks/{webhookId}/test/",
            post_with(test_webhook, test_webhook_docs),
        )
        .with_path_items(|item| item.tag("Webhooks"))
}
