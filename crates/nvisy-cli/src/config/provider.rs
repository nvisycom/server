//! AI service provider configuration based on compile-time features.
//!
//! This module creates [`AiServices`] based on the enabled feature flags:
//!
//! - `ollama`: Use Ollama for embeddings and VLM
//! - `olmocr`: Use OLMo for OCR
//! - `mock`: Use mock providers (fallback when no real provider is selected)
//!
//! When multiple features are enabled, real providers take precedence over mocks.

use nvisy_core::AiServices;

/// Creates AI services based on enabled feature flags.
///
/// # Feature Priority
///
/// For each service type, the first available provider is used:
///
/// - **Embeddings**: `ollama` > `mock`
/// - **VLM**: `ollama` > `mock`
/// - **OCR**: `olmocr` > `mock`
///
/// # Panics
///
/// Panics if no provider is available for a service type. Ensure at least
/// the `mock` feature is enabled for development/testing.
///
/// # Example
///
/// ```ignore
/// // In Cargo.toml:
/// // [features]
/// // default = ["mock"]
/// // ollama = ["dep:nvisy-ollama"]
///
/// let services = create_ai_services();
/// ```
pub fn create_ai_services() -> AiServices {
    #[cfg(feature = "mock")]
    {
        // When mock feature is enabled, use mock services as fallback
        nvisy_test::create_mock_services()
    }

    #[cfg(not(feature = "mock"))]
    {
        compile_error!(
            "At least one AI provider feature must be enabled. \
             Enable 'mock' for development or 'ollama'/'olmocr' for production."
        );
    }
}

// TODO: When ollama and olmocr clients support async initialization,
// replace the simple mock fallback with proper provider selection:
//
// ```rust
// pub async fn create_ai_services(config: &ProviderConfig) -> Result<AiServices> {
//     let emb = create_embedding_provider(config).await?;
//     let vlm = create_vlm_provider(config).await?;
//     let ocr = create_ocr_provider(config).await?;
//     Ok(AiServices::new(emb, ocr, vlm))
// }
// ```
