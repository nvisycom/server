//! Webhook delivery provider trait and request/response types.

mod context;
mod request;
mod response;

pub use context::WebhookContext;
use nvisy_core::health::HealthCheck;
pub use request::{WebhookPayload, WebhookRequest};
pub use response::WebhookResponse;

use crate::Result;

/// Core trait for webhook delivery operations.
///
/// Implement this trait to create custom webhook delivery providers. A provider
/// reports its own health through [`HealthCheck`]: one that delivers directly
/// has no backing service and reports healthy, while one fronting an external
/// delivery service (such as Svix) reports that service's health.
#[async_trait::async_trait]
pub trait WebhookProvider: HealthCheck {
    /// Delivers a webhook payload to the specified endpoint.
    async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse>;
}
