//! Transformer node types for processing and transforming data.

mod chunk;
mod embedding;
mod enrich;
mod partition;

pub use chunk::{ChunkConfig, ChunkStrategy};
pub use embedding::EmbeddingConfig;
pub use enrich::EnrichConfig;
pub use partition::PartitionConfig;

use serde::{Deserialize, Serialize};

/// Transformer node configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformerConfig {
    /// Partition data into multiple outputs.
    Partition(PartitionConfig),
    /// Chunk content into smaller pieces.
    Chunk(ChunkConfig),
    /// Enrich data with additional information.
    Enrich(EnrichConfig),
    /// Generate vector embeddings.
    Embedding(EmbeddingConfig),
}
