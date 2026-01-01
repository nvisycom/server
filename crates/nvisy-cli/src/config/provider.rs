//! AI service provider configuration based on compile-time features.
//!
//! This module creates [`InferenceService`] based on the enabled feature flags:
//!
//! - `ollama`: Use Ollama for embeddings, VLM, and OCR
//! - `mock`: Use mock providers (fallback when no real provider is selected)
//!
//! When multiple features are enabled, real providers take precedence over mocks.

#[cfg(any(feature = "mock", feature = "ollama"))]
use nvisy_service::inference::InferenceService;

#[cfg(any(feature = "mock", feature = "ollama"))]
use super::Cli;

// Compile-time check: at least one AI backend must be enabled
#[cfg(not(any(feature = "mock", feature = "ollama")))]
compile_error!(
    "At least one AI provider backend must be enabled. \
     Enable either the 'mock' (for testing) or 'ollama' (for production) feature. \
     Example: cargo build --features ollama"
);

/// Creates inference service based on enabled feature flags and CLI configuration.
///
/// # Feature Priority
///
/// When multiple features are enabled, `ollama` takes precedence over `mock`.
///
/// # Errors
///
/// Returns an error if a provider cannot be initialized.
#[cfg(feature = "ollama")]
pub fn create_services(cli: &Cli) -> anyhow::Result<InferenceService> {
    use anyhow::Context;
    use nvisy_ollama::OllamaClient;
    let client = OllamaClient::new(cli.ollama.clone()).context("failed to create Ollama client")?;
    Ok(InferenceService::from_provider(client))
}

/// Creates inference service using mock providers for testing.
#[cfg(all(feature = "mock", not(feature = "ollama")))]
pub fn create_services(_cli: &Cli) -> anyhow::Result<InferenceService> {
    Ok(InferenceService::mock())
}
