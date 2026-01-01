//! Mock implementations of AI inference services for testing.
//!
//! This module provides a unified mock provider that implements [`InferenceProvider`].
//! These mocks return sensible defaults and are useful for unit and integration testing.
//!
//! # Feature Flag
//!
//! This module is only available when the `test-utils` feature is enabled:
//!
//! ```toml
//! [dev-dependencies]
//! nvisy-service = { version = "...", features = ["test-utils"] }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_service::inference::{InferenceService, MockProvider, MockConfig};
//!
//! // Create with defaults using InferenceService::mock()
//! let service = InferenceService::mock();
//!
//! // Or create with custom configuration
//! let config = MockConfig {
//!     embedding_dimensions: 256,
//!     mock_text: Some("Custom OCR text".into()),
//!     mock_response: Some("Custom VLM response".into()),
//! };
//! let service = InferenceService::from_provider(MockProvider::new(config));
//! ```

use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "config")]
use clap::Args;
use jiff::SignedDuration;
use serde::{Deserialize, Serialize};

use super::{
    EmbeddingRequest, EmbeddingResponse, InferenceProvider, InferenceService, OcrRequest,
    OcrResponse, Result, ServiceHealth, SharedContext, UsageStats, VlmRequest, VlmResponse,
};

/// Configuration for the mock provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct MockConfig {
    /// Dimensions of mock embedding vectors.
    #[cfg_attr(
        feature = "config",
        arg(
            long = "mock-embedding-dimensions",
            env = "MOCK_EMBEDDING_DIMENSIONS",
            default_value = "128"
        )
    )]
    #[serde(default = "default_dimensions")]
    pub embedding_dimensions: usize,

    /// Mock text to return for OCR requests.
    #[cfg_attr(feature = "config", arg(long = "mock-text", env = "MOCK_TEXT"))]
    #[serde(default)]
    pub mock_text: Option<String>,

    /// Mock response content to return for VLM requests.
    #[cfg_attr(feature = "config", arg(long = "mock-response", env = "MOCK_RESPONSE"))]
    #[serde(default)]
    pub mock_response: Option<String>,
}

fn default_dimensions() -> usize {
    128
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            embedding_dimensions: default_dimensions(),
            mock_text: None,
            mock_response: None,
        }
    }
}

impl MockConfig {
    /// Convert this configuration into a complete set of inference services.
    pub fn into_services(self) -> InferenceService {
        MockProvider::new(self).into_services()
    }
}

/// Unified mock provider for testing.
///
/// Implements [`InferenceProvider`] trait, returning configurable mock responses
/// for all embedding, OCR, and VLM requests.
#[derive(Clone, Debug)]
pub struct MockProvider {
    config: Arc<MockConfig>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(MockConfig::default())
    }
}

impl MockProvider {
    /// Creates a new mock provider with the given configuration.
    pub fn new(config: MockConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Creates a new mock provider with custom embedding dimensions.
    pub fn with_dimensions(dimensions: usize) -> Self {
        Self::new(MockConfig {
            embedding_dimensions: dimensions,
            ..Default::default()
        })
    }

    /// Creates a new mock provider with custom OCR text.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self::new(MockConfig {
            mock_text: Some(text.into()),
            ..Default::default()
        })
    }

    /// Creates a new mock provider with custom VLM response.
    pub fn with_response(response: impl Into<String>) -> Self {
        Self::new(MockConfig {
            mock_response: Some(response.into()),
            ..Default::default()
        })
    }

    /// Convert this provider into a complete set of inference services.
    pub fn into_services(self) -> InferenceService {
        InferenceService::from_provider(self)
    }
}

#[async_trait::async_trait]
impl InferenceProvider for MockProvider {
    async fn generate_embedding(
        &self,
        context: &SharedContext,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse> {
        let start = Instant::now();

        // Generate a mock embedding vector with configured dimensions
        let mock_embedding = vec![0.1_f32; self.config.embedding_dimensions];
        let response = request.reply(mock_embedding);

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        let tokens = request.content.estimated_size() as u32 / 4;
        context
            .record(UsageStats::success(tokens, 1, processing_time))
            .await;

        Ok(response)
    }

    async fn process_ocr(
        &self,
        context: &SharedContext,
        request: &OcrRequest,
    ) -> Result<OcrResponse> {
        let start = Instant::now();

        // Return configured mock text or empty string
        let text = self.config.mock_text.clone().unwrap_or_default();
        let response = request.reply(text);

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        context
            .record(UsageStats::success(0, 1, processing_time))
            .await;

        Ok(response)
    }

    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &VlmRequest,
    ) -> Result<VlmResponse> {
        let start = Instant::now();

        // Return configured mock response or default
        let content = self
            .config
            .mock_response
            .clone()
            .unwrap_or_else(|| "Mock VLM response".to_string());

        let response = request
            .reply(content.clone())
            .with_finish_reason("stop")
            .with_confidence(0.95);

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        let runs = request.document_count() as u32;
        let tokens = content.len() as u32 / 4;
        context
            .record(UsageStats::success(tokens, runs, processing_time))
            .await;

        Ok(response)
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}
