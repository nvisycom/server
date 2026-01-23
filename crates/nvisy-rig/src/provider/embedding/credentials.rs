//! Embedding provider credentials.

use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

pub use super::super::credentials::{ApiKeyCredentials, OllamaCredentials};

/// Credentials for embedding providers.
#[derive(Debug, Clone, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EmbeddingCredentials {
    /// OpenAI credentials.
    OpenAi(ApiKeyCredentials),
    /// Cohere credentials.
    Cohere(ApiKeyCredentials),
    /// Google Gemini credentials.
    Gemini(ApiKeyCredentials),
    /// Ollama credentials.
    #[cfg(feature = "ollama")]
    Ollama(OllamaCredentials),
}

impl EmbeddingCredentials {
    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }
}
