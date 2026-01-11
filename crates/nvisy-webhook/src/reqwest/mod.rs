//! Reqwest-based HTTP client for webhook delivery.
//!
//! This module provides a reqwest-based implementation of the [`WebhookProvider`] trait.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_webhook::reqwest::{ReqwestClient, ReqwestConfig};
//! use nvisy_webhook::{WebhookRequest, WebhookService};
//!
//! // Create a client with default configuration
//! let client = ReqwestClient::default();
//!
//! // Convert to a service for dependency injection
//! let service: WebhookService = client.into_service();
//! ```

mod client;
mod config;
mod error;

pub use client::ReqwestClient;
pub use config::ReqwestConfig;
pub use error::{Error, Result};

/// Tracing target for reqwest client operations.
pub const TRACING_TARGET: &str = "nvisy_webhook::reqwest";
