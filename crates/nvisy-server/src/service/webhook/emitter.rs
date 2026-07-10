//! Webhook event emitter for publishing domain events to NATS.

use std::collections::HashMap;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::stream::{EventPublisher, WebhookStream};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::WorkspaceWebhook;
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_postgres::types::WebhookEvent;
use nvisy_webhook::provider::{WebhookContext, WebhookRequest};
use url::Url;
use uuid::Uuid;

use crate::Result;
use crate::service::CryptoService;

/// Type alias for webhook publisher.
type WebhookPublisher = EventPublisher<WebhookRequest, WebhookStream>;

/// Tracing target for webhook event emission.
const TRACING_TARGET: &str = "nvisy_server::service::webhook";

/// Default timeout for webhook delivery.
const DEFAULT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// The event details shared by every webhook request in one emission.
struct EmitContext {
    workspace_id: Uuid,
    resource_id: Uuid,
    resource_type: String,
    event: String,
    triggered_by: Option<Uuid>,
    data: Option<serde_json::Value>,
}

/// Webhook event emitter for publishing domain events.
///
/// This service queries webhooks subscribed to specific events and publishes
/// requests to NATS for asynchronous delivery.
#[derive(Clone)]
pub struct WebhookEmitter {
    pg_client: PgClient,
    nats_client: NatsClient,
    crypto: CryptoService,
}

impl WebhookEmitter {
    /// Create a new webhook emitter.
    pub fn new(pg_client: PgClient, nats_client: NatsClient, crypto: CryptoService) -> Self {
        Self {
            pg_client,
            nats_client,
            crypto,
        }
    }

    /// Emit a webhook event for a workspace.
    ///
    /// This method:
    /// 1. Queries all active webhooks subscribed to the event type
    /// 2. Creates a `WebhookRequest` for each webhook
    /// 3. Publishes the requests to NATS for asynchronous delivery
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The workspace where the event occurred
    /// * `event` - The type of event that occurred
    /// * `resource_id` - The ID of the affected resource
    /// * `triggered_by` - The account ID that triggered the event (if any)
    /// * `data` - Additional event-specific data
    #[tracing::instrument(
        skip(self, data),
        fields(
            workspace_id = %workspace_id,
            event = %event,
            resource_id = %resource_id,
        )
    )]
    pub async fn emit(
        &self,
        workspace_id: Uuid,
        event: WebhookEvent,
        resource_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        // Find all active webhooks subscribed to this event
        let mut conn = self.pg_client.get_connection().await?;
        let webhooks = conn.find_webhooks_for_event(workspace_id, event).await?;

        if webhooks.is_empty() {
            tracing::debug!(
                target: TRACING_TARGET,
                "No webhooks subscribed to event"
            );
            return Ok(0);
        }

        tracing::debug!(
            target: TRACING_TARGET,
            webhook_count = webhooks.len(),
            "Found webhooks subscribed to event"
        );

        // Build a signed request per webhook, skipping any that can't be built.
        let event_subject = event.as_subject();
        let context = EmitContext {
            workspace_id,
            resource_id,
            resource_type: event.category().to_string(),
            event: event.to_string(),
            triggered_by,
            data,
        };

        let requests: Vec<WebhookRequest> = webhooks
            .into_iter()
            .filter_map(|webhook| self.build_request(webhook, &context))
            .collect();

        let request_count = requests.len();

        if request_count == 0 {
            return Ok(0);
        }

        // Publish requests to NATS
        let publisher: WebhookPublisher = self.nats_client.event_publisher().await?;

        for request in &requests {
            // Use workspace_id.event_subject as the routing subject
            let subject = format!("{}.{}", request.context.workspace_id, event_subject);
            publisher.publish_to(&subject, request).await?;
        }

        tracing::info!(
            target: TRACING_TARGET,
            request_count,
            "Published webhook requests"
        );

        Ok(request_count)
    }

    /// Builds a signed delivery request for one webhook.
    ///
    /// Returns `None` — logging the reason — when the webhook can't be turned
    /// into a valid request (bad URL, unrecoverable secret), so a single
    /// misconfigured webhook doesn't abort the whole emission.
    fn build_request(
        &self,
        webhook: WorkspaceWebhook,
        ctx: &EmitContext,
    ) -> Option<WebhookRequest> {
        let url: Url = webhook
            .url
            .parse()
            .inspect_err(|err| {
                tracing::warn!(
                    target: TRACING_TARGET,
                    webhook_id = %webhook.id,
                    url = %webhook.url,
                    error = %err,
                    "Skipping webhook with invalid URL"
                );
            })
            .ok()?;

        let secret = self.decrypt_secret(&webhook, ctx.workspace_id)?;

        let mut context = WebhookContext::new(webhook.id, ctx.workspace_id, ctx.resource_id)
            .with_resource_type(&ctx.resource_type);
        if let Some(account_id) = ctx.triggered_by {
            context = context.with_account(account_id);
        }
        if let Some(metadata) = &ctx.data {
            context = context.with_metadata(metadata.clone());
        }

        let mut request =
            WebhookRequest::new(url, &ctx.event, format!("Event: {}", ctx.event), context)
                .with_timeout(DEFAULT_DELIVERY_TIMEOUT)
                .with_secret(secret);

        if let Some(headers) = parse_headers(&webhook.headers) {
            request = request.with_headers(headers);
        }

        Some(request)
    }

    /// Decrypts a webhook's stored signing secret, returning `None` (and logging)
    /// if it can't be recovered — the request is signed or not sent at all.
    fn decrypt_secret(&self, webhook: &WorkspaceWebhook, workspace_id: Uuid) -> Option<String> {
        let plaintext = self
            .crypto
            .decrypt(workspace_id, &webhook.encrypted_secret)
            .inspect_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    webhook_id = %webhook.id,
                    error = %err,
                    "Skipping webhook with undecryptable secret"
                );
            })
            .ok()?;

        String::from_utf8(plaintext)
            .inspect_err(|err| {
                tracing::error!(
                    target: TRACING_TARGET,
                    webhook_id = %webhook.id,
                    error = %err,
                    "Skipping webhook with non-UTF-8 secret"
                );
            })
            .ok()
    }

    /// Emit a document created event.
    #[inline]
    pub async fn emit_document_created(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::DocumentCreated,
            document_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a document updated event.
    #[inline]
    pub async fn emit_document_updated(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::DocumentUpdated,
            document_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a document deleted event.
    #[inline]
    pub async fn emit_document_deleted(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::DocumentDeleted,
            document_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a file created event.
    #[inline]
    pub async fn emit_file_created(
        &self,
        workspace_id: Uuid,
        file_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::FileCreated,
            file_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a file updated event.
    #[inline]
    pub async fn emit_file_updated(
        &self,
        workspace_id: Uuid,
        file_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::FileUpdated,
            file_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a file deleted event.
    #[inline]
    pub async fn emit_file_deleted(
        &self,
        workspace_id: Uuid,
        file_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::FileDeleted,
            file_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a member added event.
    #[inline]
    pub async fn emit_member_added(
        &self,
        workspace_id: Uuid,
        member_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::MemberAdded,
            member_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a member updated event.
    #[inline]
    pub async fn emit_member_updated(
        &self,
        workspace_id: Uuid,
        member_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::MemberUpdated,
            member_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a member deleted event.
    #[inline]
    pub async fn emit_member_deleted(
        &self,
        workspace_id: Uuid,
        member_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::MemberDeleted,
            member_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a connection created event.
    #[inline]
    pub async fn emit_connection_created(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::ConnectionCreated,
            connection_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a connection updated event.
    #[inline]
    pub async fn emit_connection_updated(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::ConnectionUpdated,
            connection_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a connection deleted event.
    #[inline]
    pub async fn emit_connection_deleted(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::ConnectionDeleted,
            connection_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a connection synced event.
    #[inline]
    pub async fn emit_connection_synced(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::ConnectionSynced,
            connection_id,
            triggered_by,
            data,
        )
        .await
    }

    /// Emit a connection desynced event.
    #[inline]
    pub async fn emit_connection_desynced(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        triggered_by: Option<Uuid>,
        data: Option<serde_json::Value>,
    ) -> Result<usize> {
        self.emit(
            workspace_id,
            WebhookEvent::ConnectionDesynced,
            connection_id,
            triggered_by,
            data,
        )
        .await
    }
}

/// Extracts a webhook's custom headers from its stored JSON, keeping only
/// string values. Returns `None` when there are no usable headers.
fn parse_headers(headers: &serde_json::Value) -> Option<HashMap<String, String>> {
    let map: HashMap<String, String> = headers
        .as_object()?
        .iter()
        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
        .collect();

    (!map.is_empty()).then_some(map)
}
