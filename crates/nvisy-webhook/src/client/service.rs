//! Webhook service wrapper.

use std::fmt;
use std::sync::Arc;

use super::{ServiceHealth, WebhookProvider, WebhookRequest, WebhookResponse};
use crate::Result;

/// Webhook service wrapper for dependency injection.
///
/// Wraps any [`WebhookProvider`] in an `Arc` for cheap cloning across tasks.
#[derive(Clone)]
pub struct WebhookService {
    inner: Arc<dyn WebhookProvider>,
}

impl fmt::Debug for WebhookService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookService").finish_non_exhaustive()
    }
}

impl WebhookService {
    /// Create a new webhook service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: WebhookProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
        }
    }

    /// Delivers a webhook payload to the specified endpoint.
    pub async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse> {
        self.inner.deliver(request).await
    }

    /// Performs a health check on the underlying webhook provider.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
