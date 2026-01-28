//! Shared credential types for AI providers.

use nvisy_core::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

/// API key credentials for AI providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCredentials {
    /// API key.
    pub api_key: String,
}

/// Unified credentials for all AI providers.
///
/// This enum contains credentials for all supported AI providers. The same
/// credentials can be used for both completion and embedding operations,
/// depending on the provider's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Credentials {
    /// OpenAI credentials (supports completion and embedding).
    OpenAi(ApiKeyCredentials),
    /// Anthropic credentials (completion only).
    Anthropic(ApiKeyCredentials),
    /// Cohere credentials (supports completion and embedding).
    Cohere(ApiKeyCredentials),
    /// Google Gemini credentials (supports completion and embedding).
    Gemini(ApiKeyCredentials),
    /// Perplexity credentials (completion only).
    Perplexity(ApiKeyCredentials),
}

impl Credentials {
    /// Returns the provider name as a string.
    pub fn provider(&self) -> &'static str {
        self.into()
    }

    /// Returns true if this provider supports completion.
    pub fn supports_completion(&self) -> bool {
        // All providers support completion
        true
    }

    /// Returns true if this provider supports embedding.
    pub fn supports_embedding(&self) -> bool {
        match self {
            Self::OpenAi(_) | Self::Cohere(_) | Self::Gemini(_) => true,
            Self::Anthropic(_) | Self::Perplexity(_) => false,
        }
    }

    /// Validates that credentials support embedding, returning an error if not.
    pub fn require_embedding_support(&self) -> Result<(), Error> {
        if self.supports_embedding() {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::InvalidInput)
                .with_message(format!("{} does not support embeddings", self.provider())))
        }
    }
}
