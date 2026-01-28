#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod service;

pub mod request;
pub mod response;

#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub mod reqwest;

pub use nvisy_core::{Error, ErrorKind, Result, ServiceHealth, ServiceStatus};
pub use request::{WebhookContext, WebhookPayload, WebhookRequest};
pub use response::WebhookResponse;
pub use service::WebhookService;

/// Tracing target for webhook operations.
pub const TRACING_TARGET: &str = "nvisy_service::webhook";

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
