//! Completion provider credentials.

use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

pub use super::super::credentials::{ApiKeyCredentials, OllamaCredentials};

/// Credentials for completion providers.
#[derive(Debug, Clone, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompletionCredentials {
    /// OpenAI credentials.
    OpenAi(ApiKeyCredentials),
    /// Anthropic credentials.
    Anthropic(ApiKeyCredentials),
    /// Cohere credentials.
    Cohere(ApiKeyCredentials),
    /// Google Gemini credentials.
    Gemini(ApiKeyCredentials),
    /// Perplexity credentials.
    Perplexity(ApiKeyCredentials),
    /// Ollama credentials (local, no API key required).
    Ollama(OllamaCredentials),
}

impl CompletionCredentials {
    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }
}
