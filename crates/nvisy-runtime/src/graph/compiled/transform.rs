//! Compiled transform node types.
//!
//! Processors are the runtime representation of transform nodes. Each processor
//! encapsulates the logic and dependencies needed to execute a specific transform.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::Agents;
use nvisy_rig::provider::EmbeddingProvider;

use crate::error::Result;
use crate::graph::transform::{
    ChunkStrategy, DeriveTask, EnrichTask, ExtractTask, PartitionStrategy,
};

/// Compiled transform node - ready to process data.
///
/// Each variant wraps a dedicated processor that encapsulates
/// the transform logic and any required external dependencies.
#[derive(Debug)]
pub enum CompiledTransform {
    /// Partition documents into elements.
    Partition(PartitionProcessor),
    /// Chunk content into smaller pieces.
    Chunk(ChunkProcessor),
    /// Generate vector embeddings.
    Embedding(EmbeddingProcessor),
    /// Enrich elements with metadata/descriptions.
    Enrich(EnrichProcessor),
    /// Extract structured data or convert formats.
    Extract(ExtractProcessor),
    /// Generate new content from input.
    Derive(DeriveProcessor),
}

impl CompiledTransform {
    /// Processes input data through the transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
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

// ============================================================================
// Partition Processor
// ============================================================================

/// Processor for partitioning documents into elements.
#[derive(Debug)]
pub struct PartitionProcessor {
    /// Partitioning strategy to use.
    strategy: PartitionStrategy,
    /// Whether to include page break markers.
    include_page_breaks: bool,
    /// Whether to discard unsupported element types.
    discard_unsupported: bool,
}

impl PartitionProcessor {
    /// Creates a new partition processor.
    pub fn new(
        strategy: PartitionStrategy,
        include_page_breaks: bool,
        discard_unsupported: bool,
    ) -> Self {
        Self {
            strategy,
            include_page_breaks,
            discard_unsupported,
        }
    }

    /// Returns the partitioning strategy.
    pub fn strategy(&self) -> PartitionStrategy {
        self.strategy
    }

    /// Returns whether page breaks are included.
    pub fn include_page_breaks(&self) -> bool {
        self.include_page_breaks
    }

    /// Returns whether unsupported types are discarded.
    pub fn discard_unsupported(&self) -> bool {
        self.discard_unsupported
    }

    /// Processes input data through the partition transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement document partitioning based on strategy
        // For now, pass through unchanged
        Ok(input)
    }
}

// ============================================================================
// Chunk Processor
// ============================================================================

/// Processor for chunking content into smaller pieces.
pub struct ChunkProcessor {
    /// Chunking strategy to use.
    strategy: ChunkStrategy,
    /// Whether to use LLM-powered contextual chunking.
    contextual_chunking: bool,
    /// Agents for contextual chunking (if enabled).
    agents: Option<Agents>,
}

impl ChunkProcessor {
    /// Creates a new chunk processor without contextual chunking.
    pub fn new(strategy: ChunkStrategy) -> Self {
        Self {
            strategy,
            contextual_chunking: false,
            agents: None,
        }
    }

    /// Creates a new chunk processor with contextual chunking enabled.
    pub fn with_contextual_chunking(strategy: ChunkStrategy, agents: Agents) -> Self {
        Self {
            strategy,
            contextual_chunking: true,
            agents: Some(agents),
        }
    }

    /// Returns the chunking strategy.
    pub fn strategy(&self) -> &ChunkStrategy {
        &self.strategy
    }

    /// Returns whether contextual chunking is enabled.
    pub fn contextual_chunking(&self) -> bool {
        self.contextual_chunking
    }

    /// Processes input data through the chunk transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement chunking based on strategy
        // If contextual_chunking is enabled, use agents for context generation
        Ok(input)
    }
}

impl std::fmt::Debug for ChunkProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkProcessor")
            .field("strategy", &self.strategy)
            .field("contextual_chunking", &self.contextual_chunking)
            .field("has_agents", &self.agents.is_some())
            .finish()
    }
}

// ============================================================================
// Embedding Processor
// ============================================================================

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

    /// Processes input data through the embedding transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
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

// ============================================================================
// Enrich Processor
// ============================================================================

/// Processor for enriching elements with metadata/descriptions.
pub struct EnrichProcessor {
    /// Agents for enrichment tasks.
    agents: Agents,
    /// The enrichment task to perform.
    task: EnrichTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl EnrichProcessor {
    /// Creates a new enrich processor.
    pub fn new(agents: Agents, task: EnrichTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
            task,
            override_prompt,
        }
    }

    /// Returns the enrichment task.
    pub fn task(&self) -> &EnrichTask {
        &self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }

    /// Processes input data through the enrich transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement enrichment using agents
        // Use self.agents.vision_agent for image tasks
        // Use self.agents.table_agent for table tasks
        let _ = &self.agents; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for EnrichProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrichProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Extract Processor
// ============================================================================

/// Processor for extracting structured data or converting formats.
pub struct ExtractProcessor {
    /// Agents for extraction tasks.
    agents: Agents,
    /// The extraction task to perform.
    task: ExtractTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl ExtractProcessor {
    /// Creates a new extract processor.
    pub fn new(agents: Agents, task: ExtractTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
            task,
            override_prompt,
        }
    }

    /// Returns the extraction task.
    pub fn task(&self) -> &ExtractTask {
        &self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }

    /// Processes input data through the extract transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement extraction using agents
        // Use self.agents.text_analysis_agent for NER, keywords, classification, sentiment
        // Use self.agents.table_agent for table conversion
        // Use self.agents.structured_output_agent for JSON conversion
        let _ = &self.agents; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for ExtractProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Derive Processor
// ============================================================================

/// Processor for generating new content from input.
pub struct DeriveProcessor {
    /// Agents for derivation tasks.
    agents: Agents,
    /// The derivation task to perform.
    task: DeriveTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl DeriveProcessor {
    /// Creates a new derive processor.
    pub fn new(agents: Agents, task: DeriveTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
            task,
            override_prompt,
        }
    }

    /// Returns the derivation task.
    pub fn task(&self) -> DeriveTask {
        self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }

    /// Processes input data through the derive transform.
    pub async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement derivation using agents
        // Use self.agents.text_generation_agent for summarization and title generation
        let _ = &self.agents; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for DeriveProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeriveProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}
