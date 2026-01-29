//! Webhook event emitter for publishing domain events to NATS.

use std::collections::HashMap;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::stream::{EventPublisher, WebhookStream};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_postgres::types::WebhookEvent;
use nvisy_webhook::{WebhookContext, WebhookRequest};
use url::Url;
use uuid::Uuid;

use crate::Result;

/// Type alias for webhook publisher.
type WebhookPublisher = EventPublisher<WebhookRequest, WebhookStream>;

/// Tracing target for webhook event emission.
const TRACING_TARGET: &str = "nvisy_server::service::webhook";

/// Default timeout for webhook delivery.
const DEFAULT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Webhook event emitter for publishing domain events.
///
/// This service queries webhooks subscribed to specific events and publishes
/// requests to NATS for asynchronous delivery.
#[derive(Clone)]
pub struct WebhookEmitter {
    pg_client: PgClient,
    nats_client: NatsClient,
}

impl WebhookEmitter {
    /// Create a new webhook emitter.
    pub fn new(pg_client: PgClient, nats_client: NatsClient) -> Self {
        Self {
            pg_client,
            nats_client,
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

        // Create webhook requests
        let event_subject = event.as_subject();
        let event_str = event.to_string();
        let resource_type = event.category().to_string();

        let requests: Vec<WebhookRequest> = webhooks
            .into_iter()
            .filter_map(|webhook| {
                // Parse URL - skip invalid URLs
                let url: Url = match webhook.url.parse() {
                    Ok(u) => u,
                    Err(err) => {
                        tracing::warn!(
                            target: TRACING_TARGET,
                            webhook_id = %webhook.id,
                            url = %webhook.url,
                            error = %err,
                            "Skipping webhook with invalid URL"
                        );
                        return None;
                    }
                };

                // Build context
                let mut context = WebhookContext::new(webhook.id, workspace_id, resource_id)
                    .with_resource_type(&resource_type);

                if let Some(account_id) = triggered_by {
                    context = context.with_account(account_id);
                }

                if let Some(ref metadata) = data {
                    context = context.with_metadata(metadata.clone());
                }

                // Build request
                let mut request =
                    WebhookRequest::new(url, &event_str, format!("Event: {}", event_str), context)
                        .with_timeout(DEFAULT_DELIVERY_TIMEOUT)
                        .with_secret(webhook.secret);

                // Add custom headers from webhook config
                if !webhook.headers.is_null()
                    && let Some(obj) = webhook.headers.as_object()
                {
                    let header_map: HashMap<String, String> = obj
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect();
                    if !header_map.is_empty() {
                        request = request.with_headers(header_map);
                    }
                }

                Some(request)
            })
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
