#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! # nvisy-ollama
//!
//! Ollama client library for nvisy, providing embeddings and VLM capabilities.
//!
//! The [`OllamaClient`] implements both `EmbeddingProvider` and `VlmProvider`
//! traits from nvisy-core.

/// Tracing target for the main library.
pub const TRACING_TARGET: &str = "nvisy_ollama";

/// Tracing target for client operations.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_ollama::client";

mod client;
mod error;
pub mod provider;

pub use crate::client::{OllamaClient, OllamaConfig};
pub use crate::error::{Error, Result};
pub use crate::provider::{
    OcrRequestPayload, OcrResponsePayload, VlmRequestPayload, VlmResponsePayload,
};
