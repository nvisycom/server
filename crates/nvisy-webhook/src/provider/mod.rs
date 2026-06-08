//! Webhook delivery provider trait and request/response types.

mod context;
mod request;
mod response;

pub use context::WebhookContext;
pub use request::{WebhookPayload, WebhookRequest};
pub use response::WebhookResponse;

use crate::Result;
use crate::client::ServiceHealth;

/// Core trait for webhook delivery operations.
///
/// Implement this trait to create custom webhook delivery providers.
#[async_trait::async_trait]
pub trait WebhookProvider: Send + Sync {
    /// Delivers a webhook payload to the specified endpoint.
    async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse>;

    /// Performs a health check on the webhook provider.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
