//! Embedding processor.

use nvisy_dal::datatype::AnyDataValue;
use nvisy_rig::provider::EmbeddingProvider;

use super::Process;
use crate::error::Result;

/// Processor for generating vector embeddings.
pub struct EmbeddingProcessor {
    /// The embedding provider for generating embeddings.
    provider: EmbeddingProvider,
    /// Whether to L2-normalize output embeddings.
    normalize: bool,
}

impl EmbeddingProcessor {
    /// Creates a new embedding processor.
    pub fn new(provider: EmbeddingProvider, normalize: bool) -> Self {
        Self {
            provider,
            normalize,
        }
    }

    /// Returns whether normalization is enabled.
    pub fn normalize(&self) -> bool {
        self.normalize
    }
}

impl Process for EmbeddingProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement embedding generation using provider
        // For now, pass through unchanged
        let _ = &self.provider; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for EmbeddingProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingProcessor")
            .field("normalize", &self.normalize)
            .finish_non_exhaustive()
    }
}
