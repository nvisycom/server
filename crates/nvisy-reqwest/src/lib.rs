//! Reqwest-based HTTP client implementations for nvisy services.
//!
//! This crate provides reqwest-based implementations of nvisy service traits,
//! primarily the [`ReqwestClient`] for webhook delivery.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_reqwest::{ReqwestClient, ReqwestClientConfig};
//! use nvisy_webhook::{WebhookRequest, WebhookService};
//!
//! // Create a client with default configuration
//! let client = ReqwestClient::with_defaults()?;
//!
//! // Convert to a service for dependency injection
//! let service: WebhookService = client.into_service();
//!
//! // Or use directly
//! let url = Url::parse("https://example.com/webhook")?;
//! let request = WebhookRequest::test(url, webhook_id, workspace_id);
//! let response = client.deliver(&request).await?;
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod client;
mod config;
mod error;

pub use client::{ReqwestClient, TRACING_TARGET};
pub use config::ReqwestClientConfig;
pub use error::{Error, Result};
