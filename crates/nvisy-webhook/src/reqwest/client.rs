//! Reqwest-based HTTP client for webhook delivery.

use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;

use hmac::{Hmac, KeyInit, Mac};
use jiff::Timestamp;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::TracingMiddleware;
use sha2::Sha256;
use url::Url;

use super::error::Error as ReqwestError;
use super::{ReqwestConfig, TRACING_TARGET};
use crate::guard::UrlGuardExt;
use crate::provider::{WebhookProvider, WebhookRequest, WebhookResponse};
use crate::{Error, ErrorKind, Result, WebhookService};

type HmacSha256 = Hmac<Sha256>;

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
    http: Arc<ClientWithMiddleware>,
}

impl fmt::Debug for ReqwestClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            .retry_bounds(config.min_retry_interval, config.max_retry_interval)
            .build_with_max_retries(config.max_retries);

        let http = ClientBuilder::new(base_client)
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        tracing::info!(
            target: TRACING_TARGET,
            "Reqwest client created successfully"
        );

        Self {
            http: Arc::new(http),
        }
    }

    /// Converts this client into a [`WebhookService`] for use with dependency injection.
    pub fn into_service(self) -> WebhookService {
        WebhookService::new(self)
    }

    /// Signs a payload using HMAC-SHA256.
    ///
    /// The signature is computed over the raw bytes: `{timestamp}.{payload}`.
    pub(crate) fn sign_payload(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        let timestamp_bytes = timestamp.to_string();

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(timestamp_bytes.as_bytes());
        mac.update(b".");
        mac.update(payload);

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
    async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse> {
        let started_at = Timestamp::now();
        let timestamp = started_at.as_second();

        // SSRF guard: reject non-http(s) schemes, then reject the delivery if the
        // host resolves to any non-routable address (loopback, private ranges,
        // the cloud metadata endpoint, and so on).
        request.url.check_scheme()?;
        let addrs = resolve_host(&request.url).await?;
        request.url.check_resolved_addrs(addrs)?;

        // Create the payload from the request
        let payload = request.to_payload();
        let payload_bytes = serde_json::to_vec(&payload).map_err(ReqwestError::Serde)?;

        // Build the HTTP request
        let mut http_request = self
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
                http_request.header("X-Webhook-Signature", format!("sha256={signature}"));
        }

        // Add custom headers, skipping any reserved name so a webhook cannot
        // override or duplicate the signature and other server-set headers.
        for (name, value) in &request.headers {
            if is_reserved_header(name) {
                continue;
            }
            http_request = http_request.header(name, value);
        }

        // Send the request with the JSON payload
        let http_response = http_request
            .body(payload_bytes)
            .send()
            .await
            .map_err(ReqwestError::from)?;

        let status_code = http_response.status().as_u16();
        let response = WebhookResponse::new(request.request_id, status_code, started_at);

        Ok(response)
    }
}

/// Header names the delivery client sets itself, which a webhook's custom
/// headers must never override.
const RESERVED_HEADERS: [&str; 5] = [
    "content-type",
    "x-webhook-event",
    "x-webhook-timestamp",
    "x-webhook-request-id",
    "x-webhook-signature",
];

/// Returns whether a header name is reserved for server-set values.
///
/// HTTP header names are case-insensitive, so the comparison is too.
fn is_reserved_header(name: &str) -> bool {
    RESERVED_HEADERS
        .iter()
        .any(|reserved| name.eq_ignore_ascii_case(reserved))
}

/// Resolves a webhook URL's host to its socket addresses for the SSRF guard.
///
/// Uses the URL's explicit port, or the scheme default (443 for `https`, 80 for
/// `http`), since the port does not affect address classification.
async fn resolve_host(url: &Url) -> Result<Vec<IpAddr>> {
    // Build a resolvable `host:port` authority. `host_str` yields the bare host;
    // a literal IP resolves to itself.
    let host = url.host_str().ok_or_else(|| {
        Error::new(ErrorKind::InvalidEndpoint).with_message("webhook URL has no host")
    })?;
    let port = url.port_or_known_default().unwrap_or(443);
    let authority = format!("{host}:{port}");

    let addrs = tokio::net::lookup_host(authority)
        .await
        .map_err(|err| {
            Error::new(ErrorKind::InvalidEndpoint)
                .with_message("failed to resolve webhook host")
                .with_source(err)
        })?
        .map(|socket_addr| socket_addr.ip())
        .collect();

    Ok(addrs)
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
    fn test_sign_payload_deterministic() {
        let secret = "secret";
        let timestamp = 100i64;
        let payload = b"hello";

        let sig1 = ReqwestClient::sign_payload(secret, timestamp, payload);
        let sig2 = ReqwestClient::sign_payload(secret, timestamp, payload);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn reserved_headers_are_detected_case_insensitively() {
        assert!(is_reserved_header("X-Webhook-Signature"));
        assert!(is_reserved_header("x-webhook-signature"));
        assert!(is_reserved_header("Content-Type"));
        assert!(is_reserved_header("X-Webhook-Request-Id"));
        assert!(!is_reserved_header("X-Custom-Header"));
        assert!(!is_reserved_header("Authorization"));
    }

    #[test]
    fn test_client_creation() {
        let _client = ReqwestClient::default();
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = ReqwestClient::default();
        let health = client.health_check().await.unwrap();
        assert!(health.status.is_healthy());
    }
}
