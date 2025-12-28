//! AI service provider configuration based on compile-time features.
//!
//! This module creates [`AiServices`] based on the enabled feature flags:
//!
//! - `ollama`: Use Ollama for embeddings, VLM, and OCR
//! - `mock`: Use mock providers (fallback when no real provider is selected)
//!
//! When multiple features are enabled, real providers take precedence over mocks.

use nvisy_core::AiServices;

use super::Cli;

// Compile-time check: at least one AI backend must be enabled
#[cfg(not(any(feature = "mock", feature = "ollama")))]
compile_error!(
    "At least one AI provider backend must be enabled. \
     Enable either the 'mock' (for testing) or 'ollama' (for production) feature. \
     Example: cargo build --features ollama"
);

/// Creates AI services based on enabled feature flags and CLI configuration.
///
/// # Feature Priority
///
/// When multiple features are enabled, `ollama` takes precedence over `mock`.
///
/// # Errors
///
/// Returns an error if a provider cannot be initialized.
pub fn create_ai_services(cli: &Cli) -> anyhow::Result<AiServices> {
    #[cfg(feature = "ollama")]
    {
        use nvisy_ollama::OllamaClient;
        let client = OllamaClient::new(cli.ollama.clone())?;
        return Ok(client.into_services());
    }

    #[cfg(feature = "mock")]
    {
        use nvisy_core::{MockConfig, MockProvider};
        let config = MockConfig::default();
        return Ok(MockProvider::new(config).into_services());
    }

    unreachable!()
}
