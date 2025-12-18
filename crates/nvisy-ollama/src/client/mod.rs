//! Ollama client module
//!
//! This module provides the main client interface for Ollama API operations.
//! It handles authentication, request/response processing, and connection management.

mod credentials;
mod ollama_client;
mod ollama_config;

pub use credentials::OllamaCredentials;
pub use ollama_client::OllamaClient;
pub use ollama_config::{OllamaBuilder, OllamaBuilderError, OllamaConfig};
