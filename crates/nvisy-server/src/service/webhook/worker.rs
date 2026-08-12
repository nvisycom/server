//! Webhook delivery worker.
//!
//! Consumes webhook jobs from NATS, loads each webhook's current configuration,
//! and delivers a signed payload to the external endpoint.

use std::collections::HashMap;
use std::time::Duration;

use nvisy_nats::stream::{EventStream, EventSubscriber, TypedMessage, WebhookStream};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::WorkspaceWebhook;
use nvisy_postgres::query::WorkspaceWebhookRepository;
use nvisy_webhook::WebhookService;
use nvisy_webhook::provider::{WebhookContext, WebhookRequest};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::WebhookJob;
use crate::service::Infra;
use crate::{Error, Result};

/// Type alias for webhook subscriber.
type WebhookSubscriber = EventSubscriber<WebhookJob, WebhookStream>;

/// Tracing target for webhook worker operations.
const TRACING_TARGET: &str = "nvisy_server::worker::webhook";

/// Default timeout for a single webhook delivery attempt.
const DEFAULT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Consecutive failures after which a webhook is automatically disabled.
const MAX_CONSECUTIVE_FAILURES: i32 = 10;

/// How a delivery attempt should be settled by the run loop.
enum DeliveryOutcome {
    /// Delivered successfully; record success and ack.
    Delivered { webhook: WorkspaceWebhook },
    /// The job is no longer deliverable (missing or inactive webhook); ack
    /// without recording an outcome.
    Skip,
    /// Delivery failed but may succeed later; nack to retry, and record the
    /// failure only once the retry budget is exhausted so a redelivered event
    /// counts as a single failure. `webhook` is absent when it could not be
    /// loaded, in which case there is nothing to record.
    Retry {
        webhook: Option<WorkspaceWebhook>,
        error: Error,
    },
    /// Delivery failed in a way retrying cannot fix; record the failure and ack.
    Permanent {
        webhook: WorkspaceWebhook,
        error: Error,
    },
}

impl DeliveryOutcome {
    /// A retryable failure with no webhook to record against.
    fn retryable(error: Error) -> Self {
        Self::Retry {
            webhook: None,
            error,
        }
    }

    /// A retryable failure for a known webhook.
    fn retryable_for(webhook: WorkspaceWebhook, error: Error) -> Self {
        Self::Retry {
            webhook: Some(webhook),
            error,
        }
    }

    /// A permanent failure for a known webhook.
    fn permanent(webhook: WorkspaceWebhook, error: Error) -> Self {
        Self::Permanent { webhook, error }
    }
}

/// Webhook delivery worker.
///
/// This worker subscribes to the `WEBHOOKS` NATS stream and delivers
/// webhook payloads to external endpoints with HMAC-SHA256 signatures.
pub struct WebhookDeliveryWorker {
    infra: Infra,
    webhook_service: WebhookService,
}

impl WebhookDeliveryWorker {
    /// Create a new webhook worker.
    pub fn new(infra: Infra, webhook_service: WebhookService) -> Self {
        Self {
            infra,
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
        let subscriber: WebhookSubscriber = self.infra.nats.webhook_subscriber().await?;

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
                            self.settle(&mut message, &job).await;
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

    /// Delivers a job and settles the NATS message: ack on success, permanent
    /// failure, or skip; nack to retry otherwise. The failure is recorded once
    /// the retry budget is exhausted so a redelivered event counts once.
    async fn settle(&self, message: &mut TypedMessage<WebhookJob>, job: &WebhookJob) {
        let outcome = self.deliver(job).await;

        // The final attempt is the one NATS will not redeliver after.
        let is_final = matches!(
            (message.delivery_count(), WebhookStream::MAX_DELIVER),
            (Ok(count), Some(max)) if count as i64 >= max
        );

        let ack = match outcome {
            DeliveryOutcome::Delivered { webhook } => {
                if let Ok(mut conn) = self.infra.postgres.get_connection().await {
                    self.record_success(&mut conn, webhook.id).await;
                }
                true
            }
            DeliveryOutcome::Skip => true,
            DeliveryOutcome::Permanent { webhook, error } => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    webhook_id = %webhook.id,
                    error = %error,
                    "Webhook delivery permanently failed"
                );
                if let Ok(mut conn) = self.infra.postgres.get_connection().await {
                    self.record_failure(&mut conn, &webhook).await;
                }
                true
            }
            DeliveryOutcome::Retry { webhook, error } => {
                if is_final {
                    tracing::error!(
                        target: TRACING_TARGET,
                        webhook_id = %job.webhook_id,
                        error = %error,
                        "Webhook delivery abandoned after exhausting retries"
                    );
                    // Record the single failure for this event now that no
                    // further redelivery will happen, then ack to drop it.
                    if let (Some(webhook), Ok(mut conn)) =
                        (webhook, self.infra.postgres.get_connection().await)
                    {
                        self.record_failure(&mut conn, &webhook).await;
                    }
                    true
                } else {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        webhook_id = %job.webhook_id,
                        error = %error,
                        "Webhook delivery failed; will retry"
                    );
                    false
                }
            }
        };

        let settled = if ack {
            message.ack().await
        } else {
            message.nack().await
        };
        if let Err(err) = settled {
            tracing::error!(
                target: TRACING_TARGET,
                webhook_id = %job.webhook_id,
                error = %err,
                "Failed to settle webhook message"
            );
        }
    }

    /// Attempts to deliver a webhook job, returning how the job should be
    /// settled.
    ///
    /// Loads the webhook's current configuration, decrypts its signing secret,
    /// and delivers a signed payload. Delivery outcome is not recorded here: the
    /// run loop records it once the retry disposition is known, so a redelivered
    /// event is not counted as several distinct failures.
    async fn deliver(&self, job: &WebhookJob) -> DeliveryOutcome {
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                // The webhook could not even be loaded; retry later.
                return DeliveryOutcome::retryable(Error::from(err));
            }
        };

        let webhook = match conn.find_workspace_webhook_by_id(job.webhook_id).await {
            Ok(Some(webhook)) => webhook,
            Ok(None) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    webhook_id = %job.webhook_id,
                    "Skipping delivery for missing webhook"
                );
                return DeliveryOutcome::Skip;
            }
            Err(err) => return DeliveryOutcome::retryable(Error::from(err)),
        };

        if !webhook.status.is_enabled() {
            tracing::debug!(
                target: TRACING_TARGET,
                webhook_id = %job.webhook_id,
                status = %webhook.status,
                "Skipping delivery for inactive webhook"
            );
            return DeliveryOutcome::Skip;
        }

        // A malformed URL or unrecoverable secret is a configuration error that
        // no amount of retrying will fix, so treat it as a permanent failure.
        let request = match self.build_request(&webhook, job) {
            Ok(request) => request,
            Err(err) => return DeliveryOutcome::permanent(webhook, err),
        };

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            webhook_id = %webhook.id,
            event = %request.event,
            "Delivering webhook"
        );

        // A transport error (connection refused, timeout, SSRF rejection) is a
        // failure worth retrying.
        let response = match self.webhook_service.deliver(&request).await {
            Ok(response) => response,
            Err(err) => {
                let err = Error::external("webhook", format!("Delivery failed: {err}"));
                return DeliveryOutcome::retryable_for(webhook, err);
            }
        };

        if response.is_success() {
            tracing::debug!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %webhook.id,
                status_code = response.status_code,
                "Webhook delivered successfully"
            );
            return DeliveryOutcome::Delivered { webhook };
        }

        let err = Error::external(
            "webhook",
            format!("Delivery returned status {}", response.status_code),
        );

        // A 4xx other than the retryable ones is a permanent rejection: retrying
        // will not help. Everything else is transient.
        if is_permanent_failure(response.status_code) {
            tracing::warn!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %webhook.id,
                status_code = response.status_code,
                "Webhook delivery permanently rejected"
            );
            DeliveryOutcome::permanent(webhook, err)
        } else {
            tracing::warn!(
                target: TRACING_TARGET,
                request_id = %request.request_id,
                webhook_id = %webhook.id,
                status_code = response.status_code,
                "Webhook delivery returned non-success status"
            );
            DeliveryOutcome::retryable_for(webhook, err)
        }
    }

    /// Records a failed delivery, auto-disabling the webhook once it has failed
    /// too many times in a row. Recording is best-effort: a database error is
    /// logged but does not mask the delivery outcome.
    async fn record_failure(&self, conn: &mut PgConn, webhook: &WorkspaceWebhook) {
        let updated = match conn.record_webhook_failure(webhook.id).await {
            Ok(updated) => updated,
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    webhook_id = %webhook.id,
                    error = %err,
                    "Failed to record webhook failure"
                );
                return;
            }
        };

        if updated.consecutive_failures < MAX_CONSECUTIVE_FAILURES {
            return;
        }

        match conn.suspend_webhook(webhook.id).await {
            Ok(_) => tracing::warn!(
                target: TRACING_TARGET,
                webhook_id = %webhook.id,
                consecutive_failures = updated.consecutive_failures,
                "Auto-suspended webhook after repeated delivery failures"
            ),
            Err(err) => tracing::error!(
                target: TRACING_TARGET,
                webhook_id = %webhook.id,
                error = %err,
                "Failed to auto-suspend webhook after repeated failures"
            ),
        }
    }

    /// Records a successful delivery. Best-effort: a database error is logged but
    /// does not fail the job, so a successful delivery is never redelivered just
    /// because the bookkeeping write failed.
    async fn record_success(&self, conn: &mut PgConn, webhook_id: Uuid) {
        if let Err(err) = conn.record_webhook_success(webhook_id).await {
            tracing::error!(
                target: TRACING_TARGET,
                webhook_id = %webhook_id,
                error = %err,
                "Failed to record webhook success"
            );
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
            .infra
            .crypto
            .decrypt(workspace_id, &webhook.encrypted_secret)?;
        String::from_utf8(plaintext).map_err(|err| {
            Error::internal("webhook", "webhook secret is not UTF-8").with_source(err)
        })
    }
}

/// Returns whether a status code is a permanent rejection not worth retrying.
///
/// A 4xx generally means the receiver rejected the request itself, so
/// redelivering the same payload will fail again. The exceptions are the
/// transient ones — `408 Request Timeout`, `425 Too Early`, and
/// `429 Too Many Requests` — which ask the sender to try again later.
fn is_permanent_failure(status_code: u16) -> bool {
    (400..500).contains(&status_code) && !matches!(status_code, 408 | 425 | 429)
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
