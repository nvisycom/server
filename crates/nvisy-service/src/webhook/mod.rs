//! Webhook delivery types and traits.
//!
//! This module provides webhook delivery functionality:
//! - [`WebhookProvider`]: Core trait for webhook delivery implementations
//! - [`WebhookService`]: Service wrapper with observability
//!
//! For an HTTP-based client implementation, see the `nvisy-reqwest` crate.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_service::webhook::{WebhookPayload, WebhookProvider, WebhookService};
//!
//! // Create a service from any WebhookProvider implementation
//! let service = WebhookService::new(my_provider);
//!
//! // Create and send a webhook
//! let payload = WebhookPayload::test(webhook_id);
//! let request = payload.into_request("https://example.com/webhook");
//! let response = service.deliver(&request).await?;
//! ```

mod service;

pub mod request;
pub mod response;

pub use request::{WebhookContext, WebhookPayload, WebhookRequest};
pub use response::WebhookResponse;
pub use service::WebhookService;

pub use crate::{Error, Result};

/// Tracing target for webhook operations.
pub const TRACING_TARGET: &str = "nvisy_service::webhook";

/// Core trait for webhook delivery operations.
///
/// Implement this trait to create custom webhook delivery providers.
#[async_trait::async_trait]
pub trait WebhookProvider: Send + Sync {
    /// Delivers a webhook payload to the specified endpoint.
    async fn deliver(&self, request: &WebhookRequest) -> Result<WebhookResponse>;
}
