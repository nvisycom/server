//! The typed LLM inference connection configuration.

use std::fmt;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::InferenceClient;
use crate::error::Result;
use crate::provider::{
    AnthropicProvider, Client, OllamaCredentials, OllamaProvider, OpenAiProvider,
};

/// Configuration for a provider reached with an API key (OpenAI, Anthropic).
///
/// The `api_key` is masked in [`Debug`], so neither this struct nor any config
/// that embeds it leaks the key.
#[derive(Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedProvider {
    /// The provider API key.
    pub api_key: String,
    /// Override the API base URL (for a compatible endpoint or a proxy). Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Default model to use when a request does not specify one. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
}

impl fmt::Debug for AuthenticatedProvider {
    /// Masks `api_key`; only its presence is shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthenticatedProvider")
            .field("api_key", &"<set>")
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .finish()
    }
}

/// Configuration for a provider reached without an API key (Ollama), addressed
/// by a caller-supplied base URL. Carries no secret, so it derives [`Debug`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnauthenticatedProvider {
    /// Base URL of the server (e.g. `http://localhost:11434`).
    pub base_url: String,
    /// Default model to use when a request does not specify one. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
}

/// A fully-typed LLM inference connection configuration.
///
/// The `provider` tag selects the variant and thereby the credential shape, so
/// an OpenAI connection cannot carry Anthropic credentials. The key-bearing
/// variants hold an [`AuthenticatedProvider`], which masks the key in `Debug`;
/// serialization exists only to persist the config encrypted at rest, never to
/// return it in API responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "provider")]
pub enum LlmConfig {
    /// OpenAI (or an OpenAI-compatible endpoint).
    #[serde(rename = "openai")]
    OpenAi(AuthenticatedProvider),
    /// Ollama, typically self-hosted.
    #[serde(rename = "ollama")]
    Ollama(UnauthenticatedProvider),
    /// Anthropic (Claude).
    #[serde(rename = "anthropic")]
    Anthropic(AuthenticatedProvider),
}

impl LlmConfig {
    /// The provider identifier for this config (`openai`, `ollama`,
    /// `anthropic`), used for the stored `provider` column and for filtering.
    ///
    /// This matches the serialized `provider` tag.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => OpenAiProvider::ID,
            Self::Ollama(_) => OllamaProvider::ID,
            Self::Anthropic(_) => AnthropicProvider::ID,
        }
    }

    /// The configured default model, if any.
    #[must_use]
    pub fn default_model(&self) -> Option<&str> {
        match self {
            Self::OpenAi(p) | Self::Anthropic(p) => p.default_model.as_deref(),
            Self::Ollama(p) => p.default_model.as_deref(),
        }
    }

    /// The caller-supplied base URL, if any. This is the endpoint the deployment
    /// policy must validate before the config is stored: OpenAI/Anthropic accept
    /// an optional override, and Ollama is always a caller-supplied URL.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        match self {
            Self::OpenAi(p) | Self::Anthropic(p) => p.base_url.as_deref(),
            Self::Ollama(p) => Some(&p.base_url),
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
            Self::OpenAi(p) => {
                OpenAiProvider::connect(&p.api_key, p.base_url.as_deref())?
                    .verify()
                    .await
            }
            Self::Ollama(p) => {
                OllamaProvider::connect(&OllamaCredentials, Some(&p.base_url))?
                    .verify()
                    .await
            }
            Self::Anthropic(p) => {
                AnthropicProvider::connect(&p.api_key, p.base_url.as_deref())?
                    .verify()
                    .await
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
            Self::OpenAi(p) => {
                OpenAiProvider::connect(&p.api_key, p.base_url.as_deref())?.model(model)
            }
            Self::Ollama(p) => {
                OllamaProvider::connect(&OllamaCredentials, Some(&p.base_url))?.model(model)
            }
            Self::Anthropic(p) => {
                AnthropicProvider::connect(&p.api_key, p.base_url.as_deref())?.model(model)
            }
        };
        Ok(client)
    }
}

/// A fully-typed inference connection configuration, across every inference kind.
///
/// Inference is a family: a language model for chat today, and other model kinds
/// (e.g. named-entity recognition) as they are added. Each kind owns its own
/// config with its own `provider` tag, and this enum is untagged, so the flat
/// payload's `provider` remains the sole discriminator — an inference kind is
/// added as a new variant with no change to the wire format or storage.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum InferenceConfig {
    /// A language model (chat/completion) connection.
    Llm(LlmConfig),
}

impl InferenceConfig {
    /// The provider identifier for this config, matching the serialized `provider`
    /// tag and used for the stored `provider` column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::Llm(config) => config.provider_id(),
        }
    }

    /// The caller-supplied base URL, if any — the endpoint the deployment policy
    /// must validate before the config is stored.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        match self {
            Self::Llm(config) => config.base_url(),
        }
    }

    /// Validates this config by building its provider client and verifying the
    /// credentials against the provider.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::Error) describing the build or verification
    /// failure when the provider does not accept the credentials.
    pub async fn validate(&self) -> Result<()> {
        match self {
            Self::Llm(config) => config.validate().await,
        }
    }
}
