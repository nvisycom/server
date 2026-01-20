//! Webhook delivery worker.
//!
//! Consumes webhook requests from NATS and delivers them to external endpoints.

use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::stream::{EventSubscriber, WebhookStream};
use nvisy_webhook::{WebhookRequest, WebhookService};
use tokio_util::sync::CancellationToken;

use crate::Result;

/// Type alias for webhook subscriber.
type WebhookSubscriber = EventSubscriber<WebhookRequest, WebhookStream>;

/// Tracing target for webhook worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::webhook";

/// Webhook delivery worker.
///
/// This worker subscribes to the `WEBHOOKS` NATS stream and delivers
/// webhook payloads to external endpoints with HMAC-SHA256 signatures.
pub struct WebhookWorker {
    nats_client: NatsClient,
    webhook_service: WebhookService,
}

impl WebhookWorker {
    /// Create a new webhook worker.
    pub fn new(nats_client: NatsClient, webhook_service: WebhookService) -> Self {
        Self {
            nats_client,
            webhook_service,
        }
    }

    /// Run the webhook worker until cancelled.
    ///
    /// This method will continuously consume webhook requests from NATS and
    /// deliver them to the configured endpoints. Logs lifecycle events
    /// (start, stop, errors) internally.
    pub async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(
            target: TRACING_TARGET,
            "Starting webhook worker"
        );

        let result = self.run_inner(cancel).await;

        match &result {
            Ok(()) => {
                tracing::info!(
                    target: TRACING_TARGET,
                    "Webhook worker stopped"
                );
            }
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %err,
                    "Webhook worker failed"
                );
            }
        }

        result
    }

    /// Internal run loop.
    async fn run_inner(&self, cancel: CancellationToken) -> Result<()> {
        let subscriber: WebhookSubscriber = self.nats_client.webhook_subscriber().await?;

        let mut stream = subscriber.subscribe().await?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(
                        target: TRACING_TARGET,
                        "Webhook worker shutdown requested"
                    );
                    break;
                }
                result = stream.next_with_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(Some(mut message)) => {
                            let request = message.payload();

                            if let Err(err) = self.deliver(request).await {
                                tracing::error!(
                                    target: TRACING_TARGET,
                                    error = %err,
                                    request_id = %request.request_id,
                                    webhook_id = %request.context.webhook_id,
                                    "Failed to deliver webhook"
                                );
                                // Nack the message for redelivery
                                if let Err(nack_err) = message.nack().await {
                                    tracing::error!(
                                        target: TRACING_TARGET,
                                        error = %nack_err,
                                        "Failed to nack message"
                                    );
                                }
                            } else {
                                // Ack successful delivery
                                if let Err(ack_err) = message.ack().await {
                                    tracing::error!(
                                        target: TRACING_TARGET,
                                        error = %ack_err,
                                        "Failed to ack message"
                                    );
                                }
                            }
                        }
                        Ok(None) => {
                            // Timeout, continue loop
                        }
                        Err(err) => {
                            tracing::error!(
                                target: TRACING_TARGET,
                                error = %err,
                                "Error receiving message from stream"
                            );
                            // Brief pause before retrying
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Deliver a webhook request.
    ///
    /// The `WebhookService` handles HMAC-SHA256 signing automatically
    /// when `request.secret` is present.
    async fn deliver(&self, request: &WebhookRequest) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            webhook_id = %request.context.webhook_id,
            event = %request.event,
            "Delivering webhook"
        );

        let response = self.webhook_service.deliver(request).await.map_err(|err| {
            crate::error::Error::external("webhook", format!("Delivery failed: {}", err))
        })?;

        if response.is_success() {
            tracing::info!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %request.context.webhook_id,
                status_code = response.status_code,
                "Webhook delivered successfully"
            );
            Ok(())
        } else {
            tracing::warn!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %request.context.webhook_id,
                status_code = response.status_code,
                "Webhook delivery returned non-success status"
            );
            // Return error to trigger nack/retry
            Err(crate::error::Error::external(
                "webhook",
                format!("Delivery returned status {}", response.status_code),
            ))
        }
    }
}
