#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Tracing target for the main library
pub const TRACING_TARGET: &str = "nvisy_ollama";

/// Tracing target for client operations
pub const TRACING_TARGET_CLIENT: &str = "nvisy_ollama::client";

/// Tracing target for API operations
pub const TRACING_TARGET_API: &str = "nvisy_ollama::api";

mod client;
mod error;
#[doc(hidden)]
pub mod prelude;

pub use crate::client::{OllamaClient, OllamaConfig, OllamaBuilder, OllamaCredentials};
pub use crate::error::{Error, Result};
