//! Embedding provider credentials.

use serde::{Deserialize, Serialize};

/// Credentials for embedding providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum EmbeddingCredentials {
    /// OpenAI credentials.
    OpenAi { api_key: String },
    /// Cohere credentials.
    Cohere { api_key: String },
    /// Google Gemini credentials.
    Gemini { api_key: String },
    /// Ollama credentials.
    #[cfg(feature = "ollama")]
    Ollama { base_url: String },
}
