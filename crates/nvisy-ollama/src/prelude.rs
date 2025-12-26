//! Prelude module for nvisy-ollama.
//!
//! This module re-exports the most commonly used types, traits, and functions
//! from the nvisy-ollama library. Import this module to get quick access to
//! the essential components.

pub use crate::client::{OllamaClient, OllamaConfig};
pub use crate::error::{Error, Result};
pub use crate::provider::{OllamaEmbeddingProvider, OllamaVlmProvider};
