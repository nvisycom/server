//! Ollama client module.
//!
//! This module provides the main client interface for Ollama API operations.
//! It wraps the `ollama-rs` crate for integration with nvisy-core.

mod client;
mod config;

pub use client::OllamaClient;
pub use config::OllamaConfig;
