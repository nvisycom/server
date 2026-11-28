//! Webhook service wrapper.

use std::fmt;
use std::sync::Arc;

use crate::Result;
use crate::provider::{WebhookProvider, WebhookRequest, WebhookResponse};

/// Webhook service wrapper for dependency injection.
///
/// Wraps any [`WebhookProvider`] in an `Arc` for cheap cloning across tasks.
#[derive(Clone)]
pub struct WebhookService {
    pub(super) inner: Arc<dyn WebhookProvider>,
}

impl fmt::Debug for WebhookService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookService").finish_non_exhaustive()
    }
}

impl WebhookService {
    /// Create a new webhook service wrapper.
    pub fn new(provider: impl WebhookProvider + 'static) -> Self {
        Self {
            inner: Arc::new(provider),
        }
    }

    /// Delivers a webhook payload to the specified endpoint.
    ///
    /// # Errors
    ///
    /// Propagates any error from the underlying provider's delivery (e.g. an
    /// SSRF-guard rejection or a transport failure).
    pub async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse> {
        self.inner.deliver(request).await
    }
}
