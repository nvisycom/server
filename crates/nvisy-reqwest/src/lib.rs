//! HTTP client implementations for nvisy services.
//!
//! This crate provides reqwest-based implementations of nvisy service traits,
//! primarily the [`WebhookClient`] for webhook delivery.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_reqwest::{WebhookClient, WebhookClientConfig};
//! use nvisy_service::webhook::{WebhookPayload, WebhookService};
//!
//! // Create a client with default configuration
//! let client = WebhookClient::with_defaults()?;
//!
//! // Convert to a service for dependency injection
//! let service: WebhookService = client.into_service();
//!
//! // Or use directly
//! let payload = WebhookPayload::test(webhook_id);
//! let request = payload.into_request("https://example.com/webhook");
//! let response = client.deliver(&request).await?;
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod client;
mod config;
mod error;

pub use client::{TRACING_TARGET, WebhookClient};
pub use config::WebhookClientConfig;
pub use error::{Error, Result};
