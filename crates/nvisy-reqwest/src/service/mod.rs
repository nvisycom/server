//! Webhook service implementation.
//!
//! This module implements the [`WebhookProvider`] trait for [`ReqwestClient`].

use jiff::Timestamp;
use nvisy_webhook::{ServiceHealth, WebhookProvider, WebhookRequest, WebhookResponse};

use crate::connect::{ReqwestClient, TRACING_TARGET};
use crate::error::Error;

#[async_trait::async_trait]
impl WebhookProvider for ReqwestClient {
    async fn deliver(&self, request: &WebhookRequest) -> nvisy_webhook::Result<WebhookResponse> {
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

        // Determine the timeout to use
        let timeout = request.timeout.unwrap_or_else(|| self.config().timeout());

        // Build the HTTP request
        let mut http_request = self
            .http()
            .post(request.url.as_str())
            .header("Content-Type", "application/json")
            .header("X-Webhook-Event", &request.event)
            .header("X-Webhook-Timestamp", timestamp.to_string())
            .header("X-Webhook-Request-Id", request.request_id.to_string())
            .timeout(timeout);

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

    async fn health_check(&self) -> nvisy_webhook::Result<ServiceHealth> {
        // The client is stateless and always healthy if it was created successfully
        Ok(ServiceHealth::healthy())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_webhook::ServiceStatus;

    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let client = ReqwestClient::default();
        let health = client.health_check().await.unwrap();
        assert_eq!(health.status, ServiceStatus::Healthy);
    }
}
