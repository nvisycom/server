//! Transformer node types for processing and transforming data.

mod chunk;
mod derive;
mod embedding;
mod enrich;
mod extract;
mod partition;

use std::future::Future;

pub use chunk::{Chunk, ChunkStrategy};
pub use derive::{Derive, DeriveTask};
pub use embedding::Embedding;
pub use enrich::{Enrich, EnrichTask, ImageEnrichTask, TableEnrichTask};
pub use extract::{
    AnalyzeTask, ConvertTask, Extract, ExtractTask, TableConvertTask, TextConvertTask,
};
use nvisy_dal::AnyDataValue;
pub use partition::{Partition, PartitionStrategy};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::provider::CredentialsRegistry;

/// Trait for transforming data in a workflow pipeline.
///
/// Transforms take input data items and produce output data items.
/// A single input can produce multiple outputs (e.g., chunking splits one document
/// into many chunks, or embedding generates one vector per chunk).
pub trait Transform {
    /// Transforms input data items into output data items.
    ///
    /// # Arguments
    /// * `input` - The input data items to transform
    /// * `registry` - Credentials registry for accessing external services
    ///
    /// # Returns
    /// A vector of transformed data items (may be more or fewer than input)
    fn transform(
        &self,
        input: Vec<AnyDataValue>,
        registry: &CredentialsRegistry,
    ) -> impl Future<Output = Result<Vec<AnyDataValue>>> + Send;
}

/// Transformer node variant.
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

impl Transform for Transformer {
    async fn transform(
        &self,
        input: Vec<AnyDataValue>,
        registry: &CredentialsRegistry,
    ) -> Result<Vec<AnyDataValue>> {
        match self {
            Self::Partition(t) => t.transform(input, registry).await,
            Self::Chunk(t) => t.transform(input, registry).await,
            Self::Embedding(t) => t.transform(input, registry).await,
            Self::Enrich(t) => t.transform(input, registry).await,
            Self::Extract(t) => t.transform(input, registry).await,
            Self::Derive(t) => t.transform(input, registry).await,
        }
    }
}
