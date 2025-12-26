//! Provider implementations for Ollama operations.
//!
//! This module contains the [`crate::OllamaClient`] implementations
//! of `EmbeddingProvider` and `VlmProvider` from nvisy-core.

mod embedding;
mod visual;

pub use visual::{VlmRequestPayload, VlmResponsePayload};
