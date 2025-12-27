//! Mock embedding provider for testing.

use std::time::Instant;

#[cfg(feature = "config")]
use clap::Args;
use jiff::SignedDuration;
use nvisy_core::emb::{EmbeddingProvider, Request, Response};
use nvisy_core::{Result, ServiceHealth, SharedContext, UsageStats};
use serde::{Deserialize, Serialize};

/// Configuration for the mock embedding provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct MockEmbeddingConfig {
    /// Dimensions of the mock embedding vectors.
    #[cfg_attr(feature = "config", arg(long, default_value = "128"))]
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

fn default_dimensions() -> usize {
    128
}

/// Mock embedding provider for testing.
///
/// Returns mock embeddings with configurable dimensions for all requests.
#[derive(Clone, Default, Debug)]
pub struct MockEmbeddingProvider {
    config: MockEmbeddingConfig,
}

impl MockEmbeddingProvider {
    /// Creates a new mock embedding provider with the given configuration.
    pub fn new(config: MockEmbeddingConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn generate_embedding(
        &self,
        context: &SharedContext,
        request: &Request,
    ) -> Result<Response> {
        let start = Instant::now();

        // Generate a mock embedding vector with configured dimensions
        let mock_embedding = vec![0.1_f32; self.config.dimensions];
        let response = request.reply(mock_embedding);

        // Record usage stats
        let processing_time = SignedDuration::try_from(start.elapsed()).unwrap_or_default();
        let tokens = request.content.estimated_size() as u32 / 4; // Rough token estimate
        context
            .record(UsageStats::success(tokens, 1, processing_time))
            .await;

        Ok(response)
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        Ok(ServiceHealth::healthy())
    }
}
