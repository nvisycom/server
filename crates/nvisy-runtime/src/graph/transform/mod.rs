//! Compiled transform node types.
//!
//! Processors are the runtime representation of transform nodes. Each processor
//! encapsulates the logic and dependencies needed to execute a specific transform.

mod chunk;
mod derive;
mod embedding;
mod enrich;
mod extract;
mod partition;

use std::future::Future;

pub use chunk::ChunkProcessor;
pub use derive::DeriveProcessor;
pub use embedding::EmbeddingProcessor;
pub use enrich::EnrichProcessor;
pub use extract::ExtractProcessor;
use nvisy_dal::AnyDataValue;
pub use partition::PartitionProcessor;

use crate::error::Result;

/// Trait for processing data in a workflow pipeline.
///
/// Processors are the compiled form of transforms. They take input data items
/// and produce output data items. A single input can produce multiple outputs
/// (e.g., chunking splits one document into many chunks).
pub trait Process: Send + Sync {
    /// Processes input data items into output data items.
    ///
    /// # Arguments
    /// * `input` - The input data items to process
    ///
    /// # Returns
    /// A vector of processed data items (may be more or fewer than input)
    fn process(
        &self,
        input: Vec<AnyDataValue>,
    ) -> impl Future<Output = Result<Vec<AnyDataValue>>> + Send;
}

/// Compiled transform node - ready to process data.
///
/// Each variant wraps a dedicated processor that encapsulates
/// the transform logic and any required external dependencies.
///
/// Large processor variants are boxed to avoid enum size bloat.
#[derive(Debug)]
pub enum CompiledTransform {
    /// Partition documents into elements.
    Partition(PartitionProcessor),
    /// Chunk content into smaller pieces.
    Chunk(ChunkProcessor),
    /// Generate vector embeddings.
    Embedding(EmbeddingProcessor),
    /// Enrich elements with metadata/descriptions.
    Enrich(Box<EnrichProcessor>),
    /// Extract structured data or convert formats.
    Extract(Box<ExtractProcessor>),
    /// Generate new content from input.
    Derive(DeriveProcessor),
}

impl Process for CompiledTransform {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        match self {
            Self::Partition(p) => p.process(input).await,
            Self::Chunk(p) => p.process(input).await,
            Self::Embedding(p) => p.process(input).await,
            Self::Enrich(p) => p.process(input).await,
            Self::Extract(p) => p.process(input).await,
            Self::Derive(p) => p.process(input).await,
        }
    }
}
