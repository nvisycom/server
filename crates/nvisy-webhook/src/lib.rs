#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;
mod provider;

#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub mod reqwest;

pub use client::{ServiceHealth, WebhookService};
pub use error::{BoxedError, Error, ErrorKind, Result};
pub use provider::{
    WebhookContext, WebhookPayload, WebhookProvider, WebhookRequest, WebhookResponse,
};

/// Tracing target for webhook operations.
pub const TRACING_TARGET: &str = "nvisy_webhook";
