//! Webhook service wrapper with observability.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use super::{Result, TRACING_TARGET, WebhookProvider, WebhookRequest, WebhookResponse};

/// Webhook service wrapper with observability.
///
/// This wrapper adds structured logging to any webhook delivery implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct WebhookService {
    inner: Arc<dyn WebhookProvider>,
}

impl fmt::Debug for WebhookService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookService").finish_non_exhaustive()
    }
}

impl WebhookService {
    /// Create a new webhook service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: WebhookProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
        }
    }

    /// Delivers a webhook payload to the specified endpoint.
    pub async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse> {
        let started_at = Instant::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            url = %request.url,
            has_secret = request.secret.is_some(),
            timeout_ms = request.timeout.as_millis(),
            "Delivering webhook"
        );

        let result = self.inner.deliver(request).await;
        let elapsed = started_at.elapsed();

        match &result {
            Ok(response) => {
                if response.success {
                    tracing::debug!(
                        target: TRACING_TARGET,
                        request_id = %request.request_id,
                        response_id = %response.response_id,
                        status_code = ?response.status_code,
                        elapsed_ms = elapsed.as_millis(),
                        "Webhook delivered successfully"
                    );
                } else {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        request_id = %request.request_id,
                        response_id = %response.response_id,
                        status_code = ?response.status_code,
                        error = ?response.error,
                        elapsed_ms = elapsed.as_millis(),
                        "Webhook delivery failed"
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "Webhook delivery error"
                );
            }
        }

        result
    }
}
