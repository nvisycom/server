//! AI services container for dependency injection.
//!
//! This module provides the [`AiServices`] struct which holds all AI service
//! instances (embedding, OCR, VLM) for use in application state.

use crate::emb::EmbeddingService;
use crate::ocr::OcrService;
use crate::vlm::VlmService;

/// Container for AI services.
///
/// This struct holds references to all AI services used by the application,
/// enabling dependency injection and centralized service management.
#[derive(Clone)]
pub struct AiServices {
    /// Embedding service for generating text/image embeddings.
    pub emb: EmbeddingService,
    /// OCR service for text extraction from images/documents.
    pub ocr: OcrService,
    /// VLM service for vision-language model operations.
    pub vlm: VlmService,
}

impl AiServices {
    /// Creates a new AI services container.
    ///
    /// # Parameters
    ///
    /// * `emb` - Embedding service instance
    /// * `ocr` - OCR service instance
    /// * `vlm` - VLM service instance
    pub fn new(emb: EmbeddingService, ocr: OcrService, vlm: VlmService) -> Self {
        Self { emb, ocr, vlm }
    }
}
