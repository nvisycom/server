//! AI service provider configuration based on compile-time features.
//!
//! This module creates [`AiServices`] based on the enabled feature flags:
//!
//! - `ollama`: Use Ollama for embeddings, VLM, and OCR
//! - `mock`: Use mock providers (fallback when no real provider is selected)
//!
//! When multiple features are enabled, real providers take precedence over mocks.

use nvisy_core::AiServices;
use nvisy_core::emb::EmbeddingService;
use nvisy_core::ocr::OcrService;
use nvisy_core::vlm::VlmService;

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
/// For each service type, the first available provider is used:
///
/// - **Embeddings**: `ollama` > `mock`
/// - **VLM**: `ollama` > `mock`
/// - **OCR**: `ollama` > `mock`
///
/// # Errors
///
/// Returns an error if a provider cannot be initialized.
pub fn create_ai_services(cli: &Cli) -> anyhow::Result<AiServices> {
    let emb = create_embedding_service(cli)?;
    let vlm = create_vlm_service(cli)?;
    let ocr = create_ocr_service(cli)?;

    Ok(AiServices::new(emb, ocr, vlm))
}

/// Creates the embedding service based on feature flags.
fn create_embedding_service(cli: &Cli) -> anyhow::Result<EmbeddingService> {
    // Priority: ollama > mock
    #[cfg(feature = "ollama")]
    {
        let client = nvisy_ollama::OllamaClient::new(cli.ollama.clone())?;
        return Ok(EmbeddingService::new(client));
    }

    #[cfg(all(feature = "mock", not(feature = "ollama")))]
    {
        let _ = cli;
        return Ok(nvisy_test::create_mock_embedding_service());
    }

    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("No embedding provider available"))
}

/// Creates the VLM service based on feature flags.
fn create_vlm_service(cli: &Cli) -> anyhow::Result<VlmService> {
    // Priority: ollama > mock
    #[cfg(feature = "ollama")]
    {
        let client = nvisy_ollama::OllamaClient::new(cli.ollama.clone())?;
        return Ok(VlmService::new(client));
    }

    #[cfg(all(feature = "mock", not(feature = "ollama")))]
    {
        let _ = cli;
        return Ok(nvisy_test::create_mock_vlm_service());
    }

    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("No VLM provider available"))
}

/// Creates the OCR service based on feature flags.
fn create_ocr_service(cli: &Cli) -> anyhow::Result<OcrService> {
    // Priority: ollama > mock
    #[cfg(feature = "ollama")]
    {
        let client = nvisy_ollama::OllamaClient::new(cli.ollama.clone())?;
        return Ok(OcrService::new(client));
    }

    #[cfg(all(feature = "mock", not(feature = "ollama")))]
    {
        let _ = cli;
        return Ok(nvisy_test::create_mock_ocr_service());
    }

    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("No OCR provider available"))
}
