//! Completion provider credentials.

use serde::{Deserialize, Serialize};

/// Credentials for completion providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum CompletionCredentials {
    /// OpenAI credentials.
    OpenAi { api_key: String },
    /// Anthropic credentials.
    Anthropic { api_key: String },
    /// Cohere credentials.
    Cohere { api_key: String },
    /// Google Gemini credentials.
    Gemini { api_key: String },
    /// Perplexity credentials.
    Perplexity { api_key: String },
    /// Ollama credentials (local, no API key required).
    Ollama { base_url: String },
}
