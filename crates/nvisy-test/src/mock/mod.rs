//! Mock implementations of AI services for testing.
//!
//! This module provides mock implementations of the embedding, OCR, and VLM
//! providers defined in nvisy-core. These mocks return sensible defaults
//! and are useful for unit and integration testing.

mod embedding;
mod language;
mod optical;

pub use embedding::{MockEmbeddingConfig, MockEmbeddingProvider};
pub use language::{MockLanguageConfig, MockLanguageProvider};
use nvisy_core::AiServices;
use nvisy_core::emb::EmbeddingService;
use nvisy_core::ocr::OcrService;
use nvisy_core::vlm::VlmService;
pub use optical::{MockOpticalConfig, MockOpticalProvider};

/// Creates a complete set of mock AI services for testing.
///
/// Returns an [`AiServices`] container with mock implementations of
/// embedding, OCR, and VLM services.
pub fn create_mock_services() -> AiServices {
    AiServices::new(
        create_embedding_service(),
        create_optical_service(),
        create_language_service(),
    )
}

/// Creates a mock embedding service.
fn create_embedding_service() -> EmbeddingService {
    EmbeddingService::new(MockEmbeddingProvider::default())
}

/// Creates a mock OCR service.
fn create_optical_service() -> OcrService {
    OcrService::new(MockOpticalProvider::default())
}

/// Creates a mock VLM service.
fn create_language_service() -> VlmService {
    VlmService::new(MockLanguageProvider::default())
}
