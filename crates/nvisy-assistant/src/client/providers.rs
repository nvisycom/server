//! The rig client for each provider, built from its stored config.

use rig::DynModel;
use rig::error::ProviderError;
use rig::operation::Completion;
use rig::providers::anthropic::{self, AnthropicConfig};
use rig::providers::ollama::{self, OllamaConfig};
use rig::providers::openai::{self, OpenAIConfig};

use super::config::{AuthenticatedProvider, LlmConfig, UnauthenticatedProvider};
use crate::error::{Error, Result};

/// An `OpenAI` client; also serves any OpenAI-compatible endpoint (Azure
/// `OpenAI`, a proxy) through `base_url`.
fn openai(provider: &AuthenticatedProvider) -> openai::OpenAI {
    let mut config = OpenAIConfig::new(provider.api_key.expose());
    if let Some(base_url) = &provider.base_url {
        config = config.with_base_url(base_url);
    }
    config.client()
}

/// An Anthropic (Claude) client.
fn anthropic(provider: &AuthenticatedProvider) -> anthropic::Anthropic {
    let mut config = AnthropicConfig::new(provider.api_key.expose());
    if let Some(base_url) = &provider.base_url {
        config = config.with_base_url(base_url);
    }
    config.client()
}

/// An Ollama client: self-hosted and unauthenticated, so only a base URL.
fn ollama(provider: &UnauthenticatedProvider) -> ollama::Ollama {
    OllamaConfig::new()
        .with_base_url(&provider.base_url)
        .client()
}

/// Checks `config`'s credentials, or for keyless Ollama that the daemon is
/// reachable (by listing its models).
pub(super) async fn verify(config: &LlmConfig) -> Result<()> {
    let result = match config {
        LlmConfig::OpenAi(provider) => openai(provider).verify().await,
        LlmConfig::Anthropic(provider) => anthropic(provider).verify().await,
        LlmConfig::Ollama(provider) => ollama(provider).list_models().await.map(drop),
    };
    result.map_err(|err| match err {
        ProviderError::InvalidAuthentication(_) => {
            Error::Verify("invalid authentication".to_owned())
        }
        err => Error::Verify(err.to_string()),
    })
}

/// The completion model `config`'s provider serves `model` with.
pub(super) fn completion_model(config: &LlmConfig, model: &str) -> DynModel<Completion> {
    match config {
        LlmConfig::OpenAi(provider) => openai(provider).completion(model).into(),
        LlmConfig::Anthropic(provider) => anthropic(provider).completion(model).into(),
        LlmConfig::Ollama(provider) => ollama(provider).completion(model).into(),
    }
}
