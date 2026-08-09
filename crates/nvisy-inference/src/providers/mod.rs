//! Config type and inference providers.

mod anthropic;
mod ollama;
mod openai;

use std::ops::Deref;

pub use anthropic::{AnthropicCredentials, AnthropicProvider};
pub use ollama::OllamaProvider;
pub use openai::{OpenAiCredentials, OpenAiProvider};
use rig::client::verify::VerifyClient;
use rig::client::{AgentClientExt, CompletionClient};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::{self, InferenceClient};
use crate::error::Error;

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
        /// OpenAI credentials.
        credentials: OpenAiCredentials,
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
        /// Anthropic credentials.
        credentials: AnthropicCredentials,
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

    /// Validates this config by building its provider client and verifying the
    /// credentials against the provider.
    ///
    /// Used by the connection test endpoint. Returns `Ok(())` when the provider
    /// accepts the credentials, or an [`Error`] describing the build or
    /// verification failure.
    pub async fn validate(&self) -> Result<(), Error> {
        match self {
            Self::OpenAi {
                credentials,
                base_url,
                ..
            } => client::verify(&*OpenAiProvider::connect(credentials, base_url.as_deref())?).await,
            Self::Ollama { base_url, .. } => {
                client::verify(&*OllamaProvider::connect(
                    &OllamaCredentials,
                    Some(base_url),
                )?)
                .await
            }
            Self::Anthropic {
                credentials,
                base_url,
                ..
            } => {
                client::verify(&*AnthropicProvider::connect(
                    credentials,
                    base_url.as_deref(),
                )?)
                .await
            }
        }
    }

    /// Builds a ready-to-use [`InferenceClient`] for this config.
    ///
    /// `model` overrides the configured [`default_model`](Self::default_model);
    /// if neither is set, the provider's own default applies.
    pub fn connect(&self, model: Option<&str>) -> Result<InferenceClient, Error> {
        let model = model.or_else(|| self.default_model()).unwrap_or_default();
        let client = match self {
            Self::OpenAi {
                credentials,
                base_url,
                ..
            } => OpenAiProvider::connect(credentials, base_url.as_deref())?.model(model),
            Self::Ollama { base_url, .. } => {
                OllamaProvider::connect(&OllamaCredentials, Some(base_url))?.model(model)
            }
            Self::Anthropic {
                credentials,
                base_url,
                ..
            } => AnthropicProvider::connect(credentials, base_url.as_deref())?.model(model),
        };
        Ok(client)
    }
}

/// An inference provider that builds a verifiable, prompt-capable client from
/// typed credentials.
///
/// Each provider is a newtype wrapping its rig client, which derefs to that
/// client. `client::verify` runs against it to check credentials, and `model`
/// turns it into a provider-agnostic `InferenceClient`.
pub trait Client:
    Deref<Target: Sized + VerifyClient + CompletionClient<CompletionModel: 'static>>
    + Send
    + Sync
    + 'static
{
    /// Strongly-typed credentials for this provider.
    type Credentials: Send + Sync;

    /// Unique identifier (e.g. `openai`, `anthropic`).
    const ID: &str;

    /// Build a client from credentials and an optional base-URL override.
    fn connect(credentials: &Self::Credentials, base_url: Option<&str>) -> Result<Self, Error>
    where
        Self: Sized;

    /// Turn this provider client into a provider-agnostic [`InferenceClient`]
    /// bound to `model`.
    fn model(&self, model: &str) -> InferenceClient {
        InferenceClient::new(AgentClientExt::agent(&**self, model).build())
    }
}

/// Keyless credentials for Ollama, which is typically unauthenticated.
#[derive(Debug, Clone, Copy)]
pub struct OllamaCredentials;
