//! Chunk processor.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::Agents;

use super::Process;
use crate::error::Result;
use crate::graph::definition::ChunkStrategy;

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
}

impl Process for ChunkProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
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
