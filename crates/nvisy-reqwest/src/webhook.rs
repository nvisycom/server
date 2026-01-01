//! Webhook client implementation using reqwest.

use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use nvisy_service::webhook::{WebhookProvider, WebhookRequest, WebhookResponse, WebhookService};
use reqwest::Client;
use sha2::Sha256;

use crate::error::{Error, Result};

type HmacSha256 = Hmac<Sha256>;

/// Tracing target for webhook client operations.
pub const TRACING_TARGET: &str = "nvisy_reqwest::webhook";

/// Configuration for the webhook client.
#[derive(Debug, Clone)]
pub struct WebhookClientConfig {
    /// Default timeout for webhook requests.
    pub timeout: Duration,
    /// User-Agent header to send with requests.
    pub user_agent: String,
}

impl Default for WebhookClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: format!("nvisy-webhook/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl WebhookClientConfig {
    /// Creates a new configuration with the specified timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Creates a new configuration with the specified user agent.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.timeout.is_zero() {
            return Err(Error::Config("timeout cannot be zero".into()));
        }
        if self.user_agent.is_empty() {
            return Err(Error::Config("user_agent cannot be empty".into()));
        }
        Ok(())
    }
}

/// Inner client that holds the HTTP client and configuration.
struct WebhookClientInner {
    http: Client,
    config: WebhookClientConfig,
}

impl std::fmt::Debug for WebhookClientInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookClientInner")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Webhook client for delivering webhook payloads to external endpoints.
///
/// This client implements the [`WebhookProvider`] trait and provides HTTP-based
/// webhook delivery with request signing support.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_reqwest::{WebhookClient, WebhookClientConfig};
/// use nvisy_service::webhook::WebhookRequest;
///
/// let config = WebhookClientConfig::default();
/// let client = WebhookClient::new(config)?;
///
/// let request = WebhookRequest::new("https://example.com/webhook", json!({"event": "test"}));
/// let response = client.deliver(&request).await?;
/// ```
#[derive(Clone, Debug)]
pub struct WebhookClient {
    inner: Arc<WebhookClientInner>,
}

impl WebhookClient {
    /// Creates a new webhook client with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or the HTTP client
    /// cannot be created.
    pub fn new(config: WebhookClientConfig) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            timeout_ms = config.timeout.as_millis(),
            "Creating webhook client"
        );

        config.validate()?;

        let http = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()?;

        let inner = WebhookClientInner { http, config };
        let client = Self {
            inner: Arc::new(inner),
        };

        tracing::info!(
            target: TRACING_TARGET,
            "Webhook client created successfully"
        );

        Ok(client)
    }

    /// Creates a new webhook client with default configuration.
    pub fn with_defaults() -> Result<Self> {
        Self::new(WebhookClientConfig::default())
    }

    /// Gets the client configuration.
    pub fn config(&self) -> &WebhookClientConfig {
        &self.inner.config
    }

    /// Converts this client into a [`WebhookService`] for use with dependency injection.
    pub fn into_service(self) -> WebhookService {
        WebhookService::new(self)
    }

    /// Signs a payload using HMAC-SHA256.
    ///
    /// The signature is computed over: `{timestamp}.{payload}`
    fn sign_payload(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        let signing_input = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(signing_input.as_bytes());

        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }
}

#[async_trait::async_trait]
impl WebhookProvider for WebhookClient {
    async fn deliver(&self, request: &WebhookRequest) -> nvisy_service::Result<WebhookResponse> {
        let started_at = Instant::now();
        let timestamp = jiff::Timestamp::now().as_second();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            url = %request.url,
            has_secret = request.secret.is_some(),
            "Delivering webhook"
        );

        // Serialize the payload
        let payload_bytes = serde_json::to_vec(&request.payload).map_err(|e| Error::Serde(e))?;

        // Build the HTTP request
        let mut http_request = self
            .inner
            .http
            .post(&request.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Event", "delivery")
            .header("X-Webhook-Timestamp", timestamp.to_string())
            .header("X-Webhook-Request-Id", request.request_id.to_string())
            .timeout(request.timeout);

        // Add custom headers
        for (name, value) in &request.headers {
            http_request = http_request.header(name, value);
        }

        // Sign the request if a secret is provided
        if let Some(ref secret) = request.secret {
            let signature = Self::sign_payload(secret, timestamp, &payload_bytes);
            http_request =
                http_request.header("X-Webhook-Signature", format!("sha256={signature}"));
        }

        // Send the request
        let result = http_request.body(payload_bytes).send().await;
        let elapsed = started_at.elapsed();

        match result {
            Ok(http_response) => {
                let status_code = http_response.status().as_u16();
                let success = http_response.status().is_success();

                // Collect response headers
                let mut headers = std::collections::HashMap::new();
                for (name, value) in http_response.headers() {
                    if let Ok(v) = value.to_str() {
                        headers.insert(name.to_string(), v.to_string());
                    }
                }

                // Get response body (limited to prevent memory issues)
                let body: Option<String> = http_response
                    .text()
                    .await
                    .ok()
                    .map(|b| b.chars().take(1024).collect());

                let response = if success {
                    WebhookResponse::success(request.request_id, status_code)
                } else {
                    WebhookResponse::failure(request.request_id, format!("HTTP {status_code}"))
                        .with_status_code(status_code)
                }
                .with_duration(elapsed)
                .with_headers(headers);

                let response = if let Some(b) = body {
                    response.with_body(b)
                } else {
                    response
                };

                tracing::debug!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    status_code,
                    success,
                    elapsed_ms = elapsed.as_millis(),
                    "Webhook delivery completed"
                );

                Ok(response)
            }
            Err(err) => {
                let error_message = if err.is_timeout() {
                    "Request timed out".to_string()
                } else if err.is_connect() {
                    "Connection failed".to_string()
                } else {
                    err.to_string()
                };

                tracing::warn!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    error = %error_message,
                    elapsed_ms = elapsed.as_millis(),
                    "Webhook delivery failed"
                );

                Ok(WebhookResponse::failure(request.request_id, error_message)
                    .with_duration(elapsed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = WebhookClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.user_agent.contains("nvisy-webhook"));
    }

    #[test]
    fn test_config_validation() {
        let config = WebhookClientConfig::default();
        assert!(config.validate().is_ok());

        let bad_config = WebhookClientConfig {
            timeout: Duration::ZERO,
            ..Default::default()
        };
        assert!(bad_config.validate().is_err());
    }

    #[test]
    fn test_sign_payload() {
        let secret = "test_secret";
        let timestamp = 1234567890i64;
        let payload = b"{\"event\":\"test\"}";

        let signature = WebhookClient::sign_payload(secret, timestamp, payload);

        // Signature should be a hex string (64 chars for SHA256)
        assert_eq!(signature.len(), 64);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_client_creation() {
        let config = WebhookClientConfig::default();
        let client = WebhookClient::new(config);
        assert!(client.is_ok());
    }
}
