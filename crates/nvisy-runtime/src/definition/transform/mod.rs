//! Transform definition types.
//!
//! This module contains serializable definitions for transform nodes.
//! Each transform type defines the configuration needed to perform
//! a specific data transformation in a workflow.

mod chunk;
mod derive;
mod embedding;
mod enrich;
mod extract;
mod partition;

pub use chunk::{Chunk, ChunkStrategy};
pub use derive::{Derive, DeriveTask};
pub use embedding::Embedding;
pub use enrich::{Enrich, EnrichTask, ImageEnrichTask, TableEnrichTask};
pub use extract::{
    AnalyzeTask, ConvertTask, Extract, ExtractTask, TableConvertTask, TextConvertTask,
};
pub use partition::{Partition, PartitionStrategy};
use serde::{Deserialize, Serialize};

/// Transformer node variant.
///
/// Each variant represents a different type of data transformation
/// that can be performed in a workflow pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transformer {
    /// Partition documents into elements.
    Partition(Partition),
    /// Chunk content into smaller pieces.
    Chunk(Chunk),
    /// Generate vector embeddings.
    Embedding(Embedding),
    /// Enrich elements with metadata/descriptions.
    Enrich(Enrich),
    /// Extract structured data or convert formats.
    Extract(Extract),
    /// Generate new content from input.
    Derive(Derive),
}
