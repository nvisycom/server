//! Reqwest-based HTTP client for webhook delivery.

use std::sync::Arc;
use std::time::Duration;

use hmac::{Hmac, Mac};
use jiff::Timestamp;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::TracingMiddleware;
use sha2::Sha256;

use super::{Error, ReqwestConfig, TRACING_TARGET};
use crate::{ServiceHealth, WebhookProvider, WebhookRequest, WebhookResponse, WebhookService};

type HmacSha256 = Hmac<Sha256>;

/// Inner client state.
struct ReqwestClientInner {
    http: ClientWithMiddleware,
}

/// Reqwest-based HTTP client for delivering webhook payloads to external endpoints.
///
/// This client implements the [`WebhookProvider`] trait and provides HTTP-based
/// webhook delivery with request signing, automatic retries with exponential
/// backoff, and distributed tracing.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_webhook::reqwest::{ReqwestClient, ReqwestConfig};
/// use nvisy_webhook::WebhookRequest;
/// use url::Url;
///
/// let config = ReqwestConfig::default();
/// let client = ReqwestClient::new(config);
///
/// let url = Url::parse("https://example.com/webhook")?;
/// let request = WebhookRequest::test(url, webhook_id, workspace_id);
/// let response = client.deliver(&request).await?;
/// ```
#[derive(Clone)]
pub struct ReqwestClient {
    inner: Arc<ReqwestClientInner>,
}

impl std::fmt::Debug for ReqwestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReqwestClient").finish_non_exhaustive()
    }
}

impl ReqwestClient {
    /// Creates a new reqwest client with the given configuration.
    pub fn new(config: ReqwestConfig) -> Self {
        let timeout = config.effective_timeout();
        let user_agent = config.effective_user_agent();

        tracing::debug!(
            target: TRACING_TARGET,
            timeout_ms = timeout.as_millis(),
            max_retries = config.max_retries,
            "Creating reqwest client"
        );

        let base_client = Client::builder()
            .timeout(timeout)
            .user_agent(&user_agent)
            .build()
            .expect("failed to create HTTP client");

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(config.min_retry_interval(), config.max_retry_interval())
            .build_with_max_retries(config.max_retries);

        let http = ClientBuilder::new(base_client)
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        let inner = ReqwestClientInner { http };

        tracing::info!(
            target: TRACING_TARGET,
            "Reqwest client created successfully"
        );

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Converts this client into a [`WebhookService`] for use with dependency injection.
    pub fn into_service(self) -> WebhookService {
        WebhookService::new(self)
    }

    /// Signs a payload using HMAC-SHA256.
    ///
    /// The signature is computed over: `{timestamp}.{payload}`
    pub fn sign_payload(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        let signing_input = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(signing_input.as_bytes());

        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new(ReqwestConfig::default())
    }
}

#[async_trait::async_trait]
impl WebhookProvider for ReqwestClient {
    async fn deliver(&self, request: &WebhookRequest) -> crate::Result<WebhookResponse> {
        let started_at = Timestamp::now();
        let timestamp = started_at.as_second();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            url = %request.url,
            event = %request.event,
            "Delivering webhook"
        );

        // Create the payload from the request
        let payload = request.to_payload();
        let payload_bytes = serde_json::to_vec(&payload).map_err(Error::Serde)?;

        // Build the HTTP request
        let mut http_request = self
            .inner
            .http
            .post(request.url.as_str())
            .header("Content-Type", "application/json")
            .header("X-Webhook-Event", &request.event)
            .header("X-Webhook-Timestamp", timestamp.to_string())
            .header("X-Webhook-Request-Id", request.request_id.to_string());

        // Override timeout if the request specifies one
        if let Some(timeout) = request.timeout {
            http_request = http_request.timeout(timeout);
        }

        // Add HMAC-SHA256 signature if secret is present
        if let Some(ref secret) = request.secret {
            let signature = Self::sign_payload(secret, timestamp, &payload_bytes);
            http_request =
                http_request.header("X-Webhook-Signature", format!("sha256={}", signature));
        }

        // Add custom headers
        for (name, value) in &request.headers {
            http_request = http_request.header(name, value);
        }

        // Send the request with the JSON payload
        let http_response = http_request
            .body(payload_bytes)
            .send()
            .await
            .map_err(Error::from)?;

        let status_code = http_response.status().as_u16();
        let response = WebhookResponse::new(request.request_id, status_code, started_at);

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            status_code,
            success = response.is_success(),
            "Webhook delivery completed"
        );

        Ok(response)
    }

    async fn health_check(&self) -> crate::Result<ServiceHealth> {
        Ok(ServiceHealth::healthy(Duration::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_payload() {
        let secret = "test_secret";
        let timestamp = 1234567890i64;
        let payload = b"{\"event\":\"test\"}";

        let signature = ReqwestClient::sign_payload(secret, timestamp, payload);

        // Signature should be a hex string (64 chars for SHA256)
        assert_eq!(signature.len(), 64);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_client_creation() {
        let _client = ReqwestClient::default();
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = ReqwestClient::default();
        let health = client.health_check().await.unwrap();
        assert!(health.is_healthy());
    }
}
