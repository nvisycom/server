//! Transformer node types for processing and transforming data.

mod chunk;
mod derive;
mod embedding;
mod enrich;
mod extract;
mod partition;

pub use chunk::{ChunkConfig, ChunkStrategy};
pub use derive::{DeriveConfig, DeriveTask};
pub use embedding::EmbeddingConfig;
pub use enrich::{EnrichConfig, EnrichTask, ImageEnrichTask, TableEnrichTask};
pub use extract::{
    AnalyzeTask, ConvertTask, ExtractConfig, ExtractTask, TableConvertTask, TextConvertTask,
};
pub use partition::{PartitionConfig, PartitionStrategy};
use serde::{Deserialize, Serialize};

/// Transformer node configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformerConfig {
    /// Partition documents into elements.
    Partition(PartitionConfig),
    /// Chunk content into smaller pieces.
    Chunk(ChunkConfig),
    /// Generate vector embeddings.
    Embedding(EmbeddingConfig),
    /// Enrich elements with metadata/descriptions.
    Enrich(EnrichConfig),
    /// Extract structured data or convert formats.
    Extract(ExtractConfig),
    /// Generate new content from input.
    Derive(DeriveConfig),
}
