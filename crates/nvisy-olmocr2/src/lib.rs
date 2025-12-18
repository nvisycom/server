#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Tracing target for OCR client operations.
///
/// Use this target for logging client initialization, configuration, and client-level errors.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_olmocr2::client";

mod client;
pub mod error;
#[doc(hidden)]
pub mod prelude;

pub use crate::client::{OlemCredentials, OlmBuilder, OlmClient, OlmConfig};
pub use crate::error::{Error, Result};
