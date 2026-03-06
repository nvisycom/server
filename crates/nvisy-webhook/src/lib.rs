#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;

#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub mod reqwest;

pub use client::{
    ServiceHealth, WebhookContext, WebhookPayload, WebhookProvider, WebhookRequest,
    WebhookResponse, WebhookService,
};
pub use error::{BoxedError, Error, ErrorKind, Result};

/// Tracing target for webhook operations.
pub const TRACING_TARGET: &str = "nvisy_service::webhook";
