//! Webhook delivery response types.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response from a webhook delivery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// HTTP status code from the webhook endpoint (0 if request failed before response).
    pub status_code: u16,
    /// Timestamp when the request was initiated.
    pub started_at: Timestamp,
    /// Timestamp when the response was received.
    pub finished_at: Timestamp,
}

impl WebhookResponse {
    /// Creates a new webhook response.
    pub fn new(request_id: Uuid, status_code: u16, started_at: Timestamp) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            status_code,
            started_at,
            finished_at: Timestamp::now(),
        }
    }

    /// Returns whether the delivery was successful (2xx status code).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// Calculates the response time as a duration.
    pub fn duration(&self) -> jiff::Span {
        self.started_at.until(self.finished_at).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let request_id = Uuid::new_v4();
        let started_at = Timestamp::now();
        let response = WebhookResponse::new(request_id, 200, started_at);

        assert!(response.is_success());
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn test_failure_response() {
        let started_at = Timestamp::now();

        assert!(!WebhookResponse::new(Uuid::new_v4(), 500, started_at).is_success());
        assert!(!WebhookResponse::new(Uuid::new_v4(), 404, started_at).is_success());
        assert!(!WebhookResponse::new(Uuid::new_v4(), 0, started_at).is_success());
    }
}
