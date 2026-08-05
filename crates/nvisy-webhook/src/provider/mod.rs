//! Webhook delivery provider trait and request/response types.

mod context;
mod request;
mod response;

pub use context::WebhookContext;
use nvisy_core::health::ComponentHealth;
pub use request::{WebhookPayload, WebhookRequest};
pub use response::WebhookResponse;

use crate::Result;

/// Core trait for webhook delivery operations.
///
/// Implement this trait to create custom webhook delivery providers.
#[async_trait::async_trait]
pub trait WebhookProvider: Send + Sync {
    /// Delivers a webhook payload to the specified endpoint.
    async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse>;

    /// Performs a health check on the webhook provider.
    ///
    /// Defaults to healthy for providers that deliver directly and have no
    /// backing service to probe; a provider fronting an external delivery
    /// service (such as Svix) overrides this to report that service's health.
    async fn health_check(&self) -> Result<ComponentHealth> {
        Ok(ComponentHealth::healthy("webhook"))
    }
}
