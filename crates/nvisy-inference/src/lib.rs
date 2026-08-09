#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! LLM inference provider configuration and clients.
//!
//! [`LlmConfig`] is a provider-tagged config (OpenAI, Ollama, Anthropic) that a
//! workspace connection stores encrypted at rest. [`validate`] proves a config's
//! credentials work by issuing a minimal request against the provider — used by
//! the connection `test` endpoint.

use serde::{Deserialize, Serialize};

mod client;
mod error;

pub use client::validate;
pub use error::Error;

/// A fully-typed LLM inference connection configuration.
///
/// The `provider` tag selects the variant and thereby the credential shape, so
/// an OpenAI connection cannot carry Anthropic credentials. Serialization exists
/// only to persist the config encrypted at rest, never to return it in API
/// responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "provider",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LlmConfig {
    /// OpenAI (or an OpenAI-compatible endpoint).
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
    Ollama {
        /// Base URL of the Ollama server (e.g. `http://localhost:11434`).
        base_url: String,
        /// Default model to use when a request does not specify one. Optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_model: Option<String>,
    },
    /// Anthropic (Claude).
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

/// OpenAI API credentials.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct OpenAiCredentials {
    /// OpenAI API key.
    pub api_key: String,
}

/// Anthropic API credentials.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AnthropicCredentials {
    /// Anthropic API key.
    pub api_key: String,
}

impl LlmConfig {
    /// The provider identifier for this config (`openai`, `ollama`,
    /// `anthropic`), used for the stored `provider` column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::OpenAi { .. } => "openai",
            Self::Ollama { .. } => "ollama",
            Self::Anthropic { .. } => "anthropic",
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
}
