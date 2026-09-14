//! Workspace webhook domain logic: create, read, list, update, delete, and a
//! one-off test delivery.
//!
//! Holds the webhook rules — signing-secret minting and encryption, URL
//! validation, and the lifecycle events — in one place, factored out of the
//! handler. The service owns the crypto and delivery clients because a create
//! mints and encrypts a secret and a test signs and sends a live request.

use nvisy_postgres::model::WorkspaceWebhook;
use nvisy_postgres::query::{WebhookCursor, WorkspaceWebhookRepository};
use nvisy_postgres::types::{CursorPage, CursorPagination, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn};
use nvisy_webhook::WebhookService;
use nvisy_webhook::guard::UrlGuardExt;
use nvisy_webhook::provider::{WebhookContext, WebhookRequest, WebhookResponse};
use url::Url;
use uuid::Uuid;

use crate::domain::input::{CreateWebhookInput, UpdateWebhookInput};
use crate::domain::output::CreatedWebhook;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{CryptoService, event};

/// Tracing target for webhook domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::webhook";

/// Creates, reads, updates, deletes, and tests workspace webhooks.
///
/// Holds the Postgres client (acquiring its own connection per call), the crypto
/// service (to mint, encrypt, and decrypt the signing secret), and the delivery
/// client (to send a test request). Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceWebhookService {
    postgres: PgClient,
    crypto: CryptoService,
    webhook: WebhookService,
}

impl WorkspaceWebhookService {
    /// Creates a [`WorkspaceWebhookService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, crypto: CryptoService, webhook: WebhookService) -> Self {
        Self {
            postgres,
            crypto,
            webhook,
        }
    }

    /// Creates a webhook, minting and encrypting its signing secret.
    ///
    /// The secret is generated here so it is returned once and stored only
    /// encrypted; the server decrypts it to sign each delivery. The webhook and
    /// its creation event commit together.
    ///
    /// # Errors
    ///
    /// - `BadRequest` if the URL is malformed, uses a non-`http(s)` scheme, targets
    ///   an internal address, or a supplied header is invalid.
    /// - A crypto error if minting or encrypting the signing secret fails.
    /// - A database error if the query fails.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: CreateWebhookInput,
    ) -> Result<CreatedWebhook> {
        check_webhook_url(&input.url)?;

        let secret = self.crypto.generate_secret();
        let encrypted_secret = self
            .crypto
            .encrypt(origin.workspace_id, secret.as_bytes())?;
        let new_webhook =
            input.into_model(origin.workspace_id, origin.account_id, encrypted_secret)?;

        let mut conn = self.postgres.get_connection().await?;
        let webhook = conn
            .transaction(async |conn| {
                let webhook = conn.create_workspace_webhook(new_webhook).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::WebhookCreated(event::WebhookCreated {
                        webhook_id: webhook.id,
                        webhook_name: webhook.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(webhook)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Webhook created");
        Ok(CreatedWebhook { webhook, secret })
    }

    /// Lists a workspace's webhooks, each with its creator.
    ///
    /// # Errors
    ///
    /// A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<WebhookCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceWebhook>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_webhooks(workspace_id, pagination)
            .await?)
    }

    /// Finds a webhook by id with its creator, or a `NotFound`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the webhook does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<WithAccountRef<WorkspaceWebhook>> {
        let mut conn = self.postgres.get_connection().await?;
        find_webhook(&mut conn, workspace_id, webhook_id).await
    }

    /// Updates a webhook, returning it with its creator.
    ///
    /// The update and its event commit together.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the webhook does not exist in the workspace.
    /// - `BadRequest` if a supplied URL is malformed, uses a non-`http(s)` scheme,
    ///   targets an internal address, or a supplied header is invalid.
    /// - A database error if the query fails.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        webhook_id: Uuid,
        input: UpdateWebhookInput,
    ) -> Result<WithAccountRef<WorkspaceWebhook>> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_webhook(&mut conn, origin.workspace_id, webhook_id)
            .await?
            .item;

        if let Some(url) = &input.url {
            check_webhook_url(url)?;
        }

        let update_data = input.into_model(existing.status)?;
        let webhook_name = update_data
            .display_name
            .clone()
            .unwrap_or_else(|| existing.display_name.clone());

        conn.transaction(async |conn| {
            conn.update_workspace_webhook(existing.id, update_data)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::WebhookUpdated(event::WebhookUpdated {
                    webhook_id: existing.id,
                    webhook_name,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Webhook updated");
        find_webhook(&mut conn, origin.workspace_id, webhook_id).await
    }

    /// Soft-deletes a webhook, recording the event atomically.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the webhook does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, webhook_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_webhook(&mut conn, origin.workspace_id, webhook_id)
            .await?
            .item;

        conn.transaction(async |conn| {
            conn.delete_workspace_webhook(existing.id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::WebhookDeleted(event::WebhookDeleted {
                    webhook_id: existing.id,
                    webhook_name: existing.display_name.clone(),
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Webhook deleted");
        Ok(())
    }

    /// Sends a signed test delivery to a webhook's endpoint and returns the
    /// response.
    ///
    /// The test mirrors a real delivery — decrypting the secret so the signature
    /// is present, carrying the webhook's custom headers, and including the
    /// caller's payload — but never touches stored delivery health: its outcome is
    /// returned to the caller directly, so a manual test neither counts toward the
    /// worker's auto-disable threshold nor masks a genuinely failing endpoint.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the webhook does not exist in the workspace.
    /// - `BadRequest` if the webhook's stored URL fails to parse.
    /// - `InternalServerError` if the decrypted signing secret is not valid UTF-8.
    /// - A crypto error if decrypting the signing secret fails.
    /// - A delivery error if the outbound request cannot be sent.
    /// - A database error if the query fails.
    pub async fn test(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        webhook_id: Uuid,
        payload: Option<serde_json::Value>,
    ) -> Result<WebhookResponse> {
        // Acquire and release the pooled connection before the external delivery,
        // so it is not held for the duration of that I/O.
        let webhook = {
            let mut conn = self.postgres.get_connection().await?;
            find_webhook(&mut conn, workspace_id, webhook_id)
                .await?
                .item
        };

        let url: Url = webhook
            .url
            .parse()
            .map_err(|_| ErrorKind::BadRequest.with_message("Invalid webhook URL"))?;

        let secret = String::from_utf8(
            self.crypto
                .decrypt(workspace_id, &webhook.encrypted_secret)?,
        )
        .map_err(|_| {
            ErrorKind::InternalServerError.with_message("webhook secret is not valid UTF-8")
        })?;

        let mut context =
            WebhookContext::test(webhook.id, webhook.workspace_id).with_account(account_id);
        if let Some(payload) = payload {
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
            webhook_request =
                webhook_request.with_headers(headers.into_map().into_iter().collect());
        }

        let response = self.webhook.deliver(&webhook_request).await?;
        tracing::info!(
            target: TRACING_TARGET,
            success = response.is_success(),
            "Webhook test completed",
        );
        Ok(response)
    }
}

/// Validates a webhook URL at write time: `http`/`https` scheme and, for a
/// literal-IP host, a globally routable address.
///
/// Fast feedback; the delivery worker additionally rejects hostnames that resolve
/// to non-routable addresses (which cannot be checked here without DNS).
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

/// Finds a webhook within a workspace by id, with its creator, or a `NotFound`.
async fn find_webhook(
    conn: &mut PgConn,
    workspace_id: Uuid,
    webhook_id: Uuid,
) -> Result<WithAccountRef<WorkspaceWebhook>> {
    conn.find_webhook_in_workspace_with_creator(workspace_id, webhook_id)
        .await?
        .ok_or_else(|| Error::not_found("webhook"))
}
