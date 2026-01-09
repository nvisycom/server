//! Service layer for inference operations.
//!
//! This module provides the core service infrastructure:
//! - [`InferenceProvider`] - Unified trait for AI inference operations
//! - [`InferenceService`] - High-level service wrapper with observability
//! - [`Context`] and [`SharedContext`] - Request context and usage tracking

mod context;
mod inference;

pub use context::{Context, UsageStats};
pub use inference::InferenceService;

use crate::Result;
use crate::embedding::EmbeddingProvider;
use crate::language::LanguageProvider;
use crate::optical::OpticalProvider;
use crate::types::ServiceHealth;

/// Unified trait for AI inference operations.
///
/// This trait combines [`EmbeddingProvider`], [`OpticalProvider`], and [`LanguageProvider`]
/// capabilities. Implement this trait to create custom inference providers that support
/// all three modalities.
///
/// For batch operations, use the extension traits:
/// - [`crate::EmbeddingProviderExt::generate_embedding_batch`]
/// - [`crate::OpticalProviderExt::process_ocr_batch`]
/// - [`crate::LanguageProviderExt::process_vlm_batch`]
#[async_trait::async_trait]
pub trait InferenceProvider: EmbeddingProvider + OpticalProvider + LanguageProvider {
    /// Perform a health check on the inference service.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
