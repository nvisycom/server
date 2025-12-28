//! Provider implementations for Ollama operations.
//!
//! This module contains the [`crate::OllamaClient`] implementations
//! of `EmbeddingProvider`, `VlmProvider`, and `OcrProvider` from nvisy-core.

mod embedding;
mod optical;
mod visual;

pub use optical::{OcrRequestPayload, OcrResponsePayload};
pub use visual::{VlmRequestPayload, VlmResponsePayload};
