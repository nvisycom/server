//! The typed LLM inference connection configuration.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::InferenceClient;
use crate::error::Result;
use crate::provider::{
    AnthropicProvider, Client, OllamaCredentials, OllamaProvider, OpenAiProvider,
};

/// A fully-typed LLM inference connection configuration.
///
/// The `provider` tag selects the variant and thereby the credential shape, so
/// an OpenAI connection cannot carry Anthropic credentials. Serialization exists
/// only to persist the config encrypted at rest, never to return it in API
/// responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "provider", rename_all_fields = "camelCase")]
pub enum LlmConfig {
    /// OpenAI (or an OpenAI-compatible endpoint).
    #[serde(rename = "openai")]
    OpenAi {
        /// OpenAI API key.
        api_key: String,
        /// Override the API base URL (for Azure OpenAI or a proxy). Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
        /// Default model to use when a request does not specify one. Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_model: Option<String>,
    },
    /// Ollama, typically self-hosted.
    #[serde(rename = "ollama")]
    Ollama {
        /// Base URL of the Ollama server (e.g. `http://localhost:11434`).
        base_url: String,
        /// Default model to use when a request does not specify one. Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_model: Option<String>,
    },
    /// Anthropic (Claude).
    #[serde(rename = "anthropic")]
    Anthropic {
        /// Anthropic API key.
        api_key: String,
        /// Override the API base URL. Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
        /// Default model to use when a request does not specify one. Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_model: Option<String>,
    },
}

impl LlmConfig {
    /// The provider identifier for this config (`openai`, `ollama`,
    /// `anthropic`), used for the stored `provider` column and for filtering.
    ///
    /// This matches the serialized `provider` tag.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::OpenAi { .. } => OpenAiProvider::ID,
            Self::Ollama { .. } => OllamaProvider::ID,
            Self::Anthropic { .. } => AnthropicProvider::ID,
        }
    }

    /// The configured default model, if any.
    #[must_use]
    pub fn default_model(&self) -> Option<&str> {
        match self {
            Self::OpenAi { default_model, .. }
            | Self::Ollama { default_model, .. }
            | Self::Anthropic { default_model, .. } => default_model.as_deref(),
        }
    }

    /// The caller-supplied base URL, if any. This is the endpoint the deployment
    /// policy must validate before the config is stored: OpenAI/Anthropic accept
    /// an optional override, and Ollama is always a caller-supplied URL.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        match self {
            Self::OpenAi { base_url, .. } | Self::Anthropic { base_url, .. } => base_url.as_deref(),
            Self::Ollama { base_url, .. } => Some(base_url),
        }
    }

    /// Validates this config by building its provider client and verifying the
    /// credentials against the provider.
    ///
    /// Used by the connection test endpoint. Returns `Ok(())` when the provider
    /// accepts the credentials, or an [`Error`](crate::Error) describing the
    /// build or verification failure.
    pub async fn validate(&self) -> Result<()> {
        match self {
            Self::OpenAi {
                api_key, base_url, ..
            } => {
                let provider = OpenAiProvider::connect(api_key, base_url.as_deref())?;
                provider.verify().await
            }
            Self::Ollama { base_url, .. } => {
                let provider = OllamaProvider::connect(&OllamaCredentials, Some(base_url))?;
                provider.verify().await
            }
            Self::Anthropic {
                api_key, base_url, ..
            } => {
                let provider = AnthropicProvider::connect(api_key, base_url.as_deref())?;
                provider.verify().await
            }
        }
    }

    /// Builds a ready-to-use [`InferenceClient`] for this config.
    ///
    /// `model` overrides the configured [`default_model`](Self::default_model);
    /// if neither is set, the provider's own default applies.
    pub fn connect(&self, model: Option<&str>) -> Result<InferenceClient> {
        let model = model.or_else(|| self.default_model()).unwrap_or_default();
        let client = match self {
            Self::OpenAi {
                api_key, base_url, ..
            } => OpenAiProvider::connect(api_key, base_url.as_deref())?.model(model),
            Self::Ollama { base_url, .. } => {
                OllamaProvider::connect(&OllamaCredentials, Some(base_url))?.model(model)
            }
            Self::Anthropic {
                api_key, base_url, ..
            } => AnthropicProvider::connect(api_key, base_url.as_deref())?.model(model),
        };
        Ok(client)
    }
}
