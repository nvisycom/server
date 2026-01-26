//! Shared credential types for AI providers.

use serde::{Deserialize, Serialize};

/// API key credentials for AI providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCredentials {
    /// API key.
    pub api_key: String,
}

/// Ollama credentials (local deployment, no API key required).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaCredentials {
    /// Base URL for the Ollama server.
    pub base_url: String,
}
