//! Ollama client module.
//!
//! This module provides the main client interface for Ollama API operations.
//! It wraps the `ollama-rs` crate for integration with nvisy-core.

mod ollama_client;
mod ollama_config;

pub use ollama_client::OllamaClient;
pub use ollama_config::OllamaConfig;
