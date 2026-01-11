//! Agent module for orchestrating AI-powered document processing.
//!
//! The agent is responsible for:
//! - Managing the conversation loop with the LLM
//! - Executing tool calls
//! - Proposing and applying edits
//! - Streaming responses back to the client

mod context;
mod executor;
mod prompt;

use std::sync::Arc;

pub use context::AgentContext;
pub use executor::AgentExecutor;
use futures::stream::BoxStream;
pub use prompt::PromptBuilder;
use uuid::Uuid;

use super::Session;
use crate::Result;
use crate::rag::RetrievedChunk;
use crate::service::ChatEvent;
use crate::service::provider::{ModelRef, ProviderRegistry};
use crate::tool::ToolRegistry;
use crate::tool::edit::ProposedEdit;

/// Configuration for the agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of tool call iterations.
    pub max_iterations: usize,

    /// Maximum tokens for completion.
    pub max_tokens: u32,

    /// Temperature for generation.
    pub temperature: f32,

    /// Whether to include thinking in output.
    pub include_thinking: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_tokens: 4096,
            temperature: 0.7,
            include_thinking: false,
        }
    }
}

/// The core agent that processes chat messages.
pub struct Agent {
    config: AgentConfig,
    providers: Arc<ProviderRegistry>,
    tools: Arc<ToolRegistry>,
}

impl Agent {
    /// Creates a new agent.
    pub fn new(
        config: AgentConfig,
        providers: Arc<ProviderRegistry>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            config,
            providers,
            tools,
        }
    }

    /// Processes a chat message and returns a stream of events.
    ///
    /// The `retrieved_chunks` should be pre-fetched using the RAG system.
    pub async fn process(
        &self,
        session: &Session,
        message: &str,
        retrieved_chunks: Vec<RetrievedChunk>,
        model_override: Option<&ModelRef>,
    ) -> Result<BoxStream<'static, Result<ChatEvent>>> {
        // Build context for this request
        let context = AgentContext::new(session.clone(), message.to_string(), retrieved_chunks);

        // Create executor
        let executor = AgentExecutor::new(
            self.config.clone(),
            self.providers.clone(),
            self.tools.clone(),
            context,
            model_override.cloned(),
        );

        // Run the agent loop
        executor.run().await
    }

    /// Returns proposed edits from an agent run.
    pub fn extract_edits(&self, _events: &[ChatEvent]) -> Vec<ProposedEdit> {
        // Extract proposed edits from the event stream
        // This is called after processing to collect all edits
        Vec::new()
    }
}

/// Result of an agent run.
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// The final response text.
    pub response: String,

    /// Message ID.
    pub message_id: Uuid,

    /// Proposed edits.
    pub proposed_edits: Vec<ProposedEdit>,

    /// Edits that were auto-applied.
    pub applied_edits: Vec<Uuid>,

    /// Total tokens used.
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_tokens, 4096);
        assert!(!config.include_thinking);
    }
}
