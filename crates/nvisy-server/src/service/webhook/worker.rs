//! Webhook delivery worker.
//!
//! Consumes webhook jobs from NATS, loads each webhook's current configuration,
//! and delivers a signed payload to the external endpoint.

use std::collections::HashMap;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::stream::{EventSubscriber, WebhookStream};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::WorkspaceWebhook;
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_webhook::WebhookService;
use nvisy_webhook::provider::{WebhookContext, WebhookRequest};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::WebhookJob;
use crate::service::CryptoService;
use crate::{Error, Result};

/// Type alias for webhook subscriber.
type WebhookSubscriber = EventSubscriber<WebhookJob, WebhookStream>;

/// Tracing target for webhook worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::webhook";

/// Default timeout for a single webhook delivery attempt.
const DEFAULT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Webhook delivery worker.
///
/// This worker subscribes to the `WEBHOOKS` NATS stream and delivers
/// webhook payloads to external endpoints with HMAC-SHA256 signatures.
pub struct WebhookWorker {
    pg_client: PgClient,
    nats_client: NatsClient,
    crypto: CryptoService,
    webhook_service: WebhookService,
}

impl WebhookWorker {
    /// Create a new webhook worker.
    pub fn new(
        pg_client: PgClient,
        nats_client: NatsClient,
        crypto: CryptoService,
        webhook_service: WebhookService,
    ) -> Self {
        Self {
            pg_client,
            nats_client,
            crypto,
            webhook_service,
        }
    }

    /// Run the webhook worker until cancelled.
    ///
    /// This method will continuously consume webhook jobs from NATS and
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
                            let job = message.payload().clone();

                            if let Err(err) = self.deliver(&job).await {
                                tracing::error!(
                                    target: TRACING_TARGET,
                                    error = %err,
                                    webhook_id = %job.webhook_id,
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

    /// Deliver a webhook job.
    ///
    /// Loads the webhook's current configuration, decrypts its signing secret,
    /// and delivers a signed payload. A webhook that no longer exists or is not
    /// active is skipped without error, so the job is acked rather than retried.
    async fn deliver(&self, job: &WebhookJob) -> Result<()> {
        let mut conn = self.pg_client.get_connection().await?;
        let Some(webhook) = conn.find_workspace_webhook_by_id(job.webhook_id).await? else {
            tracing::debug!(
                target: TRACING_TARGET,
                webhook_id = %job.webhook_id,
                "Skipping delivery for missing webhook"
            );
            return Ok(());
        };

        if !webhook.status.is_active() {
            tracing::debug!(
                target: TRACING_TARGET,
                webhook_id = %job.webhook_id,
                status = %webhook.status,
                "Skipping delivery for inactive webhook"
            );
            return Ok(());
        }

        let request = self.build_request(&webhook, job)?;

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            webhook_id = %webhook.id,
            event = %request.event,
            "Delivering webhook"
        );

        let response = self
            .webhook_service
            .deliver(&request)
            .await
            .map_err(|err| Error::external("webhook", format!("Delivery failed: {err}")))?;

        if response.is_success() {
            tracing::debug!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %webhook.id,
                status_code = response.status_code,
                "Webhook delivered successfully"
            );
            Ok(())
        } else {
            tracing::warn!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %webhook.id,
                status_code = response.status_code,
                "Webhook delivery returned non-success status"
            );
            Err(Error::external(
                "webhook",
                format!("Delivery returned status {}", response.status_code),
            ))
        }
    }

    /// Builds a signed delivery request from a webhook and its job.
    fn build_request(
        &self,
        webhook: &WorkspaceWebhook,
        job: &WebhookJob,
    ) -> Result<WebhookRequest> {
        let url: Url = webhook
            .url
            .parse()
            .map_err(|err| Error::internal("webhook", "invalid webhook URL").with_source(err))?;

        let secret = self.decrypt_secret(webhook, job.workspace_id)?;

        let event = job.event.to_string();
        let mut context = WebhookContext::new(webhook.id, job.workspace_id, job.resource_id)
            .with_resource_type(job.event.category());
        if let Some(account_id) = job.triggered_by {
            context = context.with_account(account_id);
        }
        if let Some(metadata) = &job.data {
            context = context.with_metadata(metadata.clone());
        }

        let mut request = WebhookRequest::new(url, &event, format!("Event: {event}"), context)
            .with_timeout(DEFAULT_DELIVERY_TIMEOUT)
            .with_secret(secret);

        if let Some(headers) = parse_headers(&webhook.headers) {
            request = request.with_headers(headers);
        }

        Ok(request)
    }

    /// Decrypts a webhook's stored signing secret under the workspace key.
    fn decrypt_secret(&self, webhook: &WorkspaceWebhook, workspace_id: Uuid) -> Result<String> {
        let plaintext = self
            .crypto
            .decrypt(workspace_id, &webhook.encrypted_secret)?;
        String::from_utf8(plaintext).map_err(|err| {
            Error::internal("webhook", "webhook secret is not UTF-8").with_source(err)
        })
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
