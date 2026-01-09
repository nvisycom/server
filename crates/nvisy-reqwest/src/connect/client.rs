//! Reqwest-based HTTP client for webhook delivery.

use std::sync::Arc;

use hmac::{Hmac, Mac};
use nvisy_webhook::WebhookService;
use reqwest::Client;
use sha2::Sha256;

use super::ReqwestConfig;

type HmacSha256 = Hmac<Sha256>;

/// Tracing target for reqwest client operations.
pub const TRACING_TARGET: &str = "nvisy_reqwest::client";

/// Inner client that holds the HTTP client and configuration.
struct ReqwestClientInner {
    http: Client,
    config: ReqwestConfig,
}

/// Reqwest-based HTTP client for delivering webhook payloads to external endpoints.
///
/// This client implements the [`WebhookProvider`] trait and provides HTTP-based
/// webhook delivery with request signing support.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_reqwest::{ReqwestClient, ReqwestConfig};
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
        f.debug_struct("ReqwestClient")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
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
            "Creating reqwest client"
        );

        let http = Client::builder()
            .timeout(timeout)
            .user_agent(&user_agent)
            .build()
            .expect("failed to create HTTP client");

        let inner = ReqwestClientInner { http, config };
        let client = Self {
            inner: Arc::new(inner),
        };

        tracing::info!(
            target: TRACING_TARGET,
            "Reqwest client created successfully"
        );

        client
    }

    /// Gets the underlying HTTP client.
    pub(crate) fn http(&self) -> &Client {
        &self.inner.http
    }

    /// Gets the client configuration.
    pub fn config(&self) -> &ReqwestConfig {
        &self.inner.config
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
        let config = ReqwestConfig::default();
        let client = ReqwestClient::new(config);
        assert!(client.config().user_agent.is_none());
    }
}
