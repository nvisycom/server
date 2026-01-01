//! Webhook delivery response types.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response from a webhook delivery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Whether the delivery was successful.
    pub success: bool,
    /// HTTP status code from the webhook endpoint.
    pub status_code: Option<u16>,
    /// Response body from the webhook endpoint (truncated if large).
    pub body: Option<String>,
    /// Error message if delivery failed.
    pub error: Option<String>,
    /// Response time in milliseconds.
    pub response_time_ms: Option<u64>,
    /// Response headers from the webhook endpoint.
    pub headers: HashMap<String, String>,
}

impl WebhookResponse {
    /// Creates a new successful webhook response.
    pub fn success(request_id: Uuid, status_code: u16) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            success: true,
            status_code: Some(status_code),
            body: None,
            error: None,
            response_time_ms: None,
            headers: HashMap::new(),
        }
    }

    /// Creates a new failed webhook response.
    pub fn failure(request_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            success: false,
            status_code: None,
            body: None,
            error: Some(error.into()),
            response_time_ms: None,
            headers: HashMap::new(),
        }
    }

    /// Sets the response time.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.response_time_ms = Some(duration.as_millis() as u64);
        self
    }

    /// Sets the response body.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Sets the status code.
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    /// Adds a response header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets multiple response headers.
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Checks if the response indicates a retryable error.
    pub fn is_retryable(&self) -> bool {
        if self.success {
            return false;
        }

        // Retry on server errors (5xx) or specific client errors
        match self.status_code {
            Some(code) => code >= 500 || code == 408 || code == 429,
            None => true, // Network errors are retryable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let request_id = Uuid::new_v4();
        let response = WebhookResponse::success(request_id, 200);

        assert!(response.success);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.status_code, Some(200));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_failure_response() {
        let request_id = Uuid::new_v4();
        let response = WebhookResponse::failure(request_id, "Connection timeout");

        assert!(!response.success);
        assert_eq!(response.request_id, request_id);
        assert!(response.status_code.is_none());
        assert_eq!(response.error, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_response_with_duration() {
        let response =
            WebhookResponse::success(Uuid::new_v4(), 200).with_duration(Duration::from_millis(150));

        assert_eq!(response.response_time_ms, Some(150));
    }

    #[test]
    fn test_is_retryable() {
        // Success is not retryable
        assert!(!WebhookResponse::success(Uuid::new_v4(), 200).is_retryable());

        // 5xx errors are retryable
        let mut response = WebhookResponse::failure(Uuid::new_v4(), "Server error");
        response.status_code = Some(500);
        assert!(response.is_retryable());

        response.status_code = Some(503);
        assert!(response.is_retryable());

        // 429 Too Many Requests is retryable
        response.status_code = Some(429);
        assert!(response.is_retryable());

        // 408 Request Timeout is retryable
        response.status_code = Some(408);
        assert!(response.is_retryable());

        // 4xx errors (except 408, 429) are not retryable
        response.status_code = Some(400);
        assert!(!response.is_retryable());

        response.status_code = Some(404);
        assert!(!response.is_retryable());

        // Network errors (no status code) are retryable
        response.status_code = None;
        assert!(response.is_retryable());
    }
}
