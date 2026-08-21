//! Webhook event emitter for publishing domain events to NATS.

use nvisy_nats::stream::{EventPublisher, WebhookStream};
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_postgres::types::WebhookEvent;
use uuid::Uuid;

use super::WebhookJob;
use crate::Result;
use crate::service::Infra;

/// Type alias for webhook publisher.
type WebhookPublisher = EventPublisher<WebhookJob, WebhookStream>;

/// Tracing target for webhook event emission.
const TRACING_TARGET: &str = "nvisy_server::service::webhook";

/// Webhook event emitter for publishing domain events.
///
/// This service queries webhooks subscribed to specific events and publishes a
/// slim delivery job per webhook to NATS for asynchronous delivery. The worker
/// loads endpoint, headers, and signing secret from the webhook's current
/// configuration at delivery time.
#[derive(Clone)]
pub struct WebhookEmitter {
    infra: Infra,
}

impl WebhookEmitter {
    /// Create a new webhook emitter.
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Emit a webhook event for a workspace.
    ///
    /// This method:
    /// 1. Queries all active webhooks subscribed to the event type
    /// 2. Creates a `WebhookJob` for each webhook
    /// 3. Publishes the jobs to NATS for asynchronous delivery
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
        let mut conn = self.infra.postgres.get_connection().await?;
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

        let event_subject = event.as_subject();
        let jobs: Vec<WebhookJob> = webhooks
            .into_iter()
            .map(|webhook| WebhookJob {
                webhook_id: webhook.id,
                workspace_id,
                event,
                resource_id,
                triggered_by,
                data: data.clone(),
            })
            .collect();

        // Publish every job before surfacing any error, so one failing publish
        // does not silently drop the webhooks that follow it in the batch.
        let publisher: WebhookPublisher = self.infra.nats.event_publisher().await?;
        let subject = format!("{workspace_id}.{event_subject}");

        let mut published = 0usize;
        let mut first_error = None;
        for job in &jobs {
            match publisher.publish_to(&subject, job).await {
                Ok(()) => published += 1,
                Err(err) => {
                    tracing::error!(
                        target: TRACING_TARGET,
                        webhook_id = %job.webhook_id,
                        error = %err,
                        "Failed to publish webhook job"
                    );
                    first_error.get_or_insert(err);
                }
            }
        }

        if let Some(err) = first_error {
            return Err(err.into());
        }

        tracing::info!(
            target: TRACING_TARGET,
            published,
            "Published webhook jobs"
        );

        Ok(published)
    }
}
