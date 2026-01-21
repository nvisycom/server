//! Agent module for orchestrating AI-powered document processing.

mod context;
mod executor;
mod prompt;

use std::sync::Arc;

pub use context::AgentContext;
pub use executor::AgentExecutor;
use futures::stream::BoxStream;
pub use prompt::PromptBuilder;
use uuid::Uuid;

use super::ChatEvent;
use crate::Result;
use crate::provider::CompletionModel;
use crate::rag::RetrievedChunk;
use crate::session::Session;
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

    /// Default completion model.
    pub default_model: CompletionModel,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_tokens: 4096,
            temperature: 0.7,
            include_thinking: false,
            default_model: CompletionModel::Ollama("llama3.2".to_string()),
        }
    }
}

/// The core agent that processes chat messages.
pub struct Agent {
    config: AgentConfig,
    tools: Arc<ToolRegistry>,
}

impl Agent {
    /// Creates a new agent.
    pub fn new(config: AgentConfig, tools: Arc<ToolRegistry>) -> Self {
        Self { config, tools }
    }

    /// Processes a chat message and returns a stream of events.
    pub async fn process(
        &self,
        session: &Session,
        message: &str,
        retrieved_chunks: Vec<RetrievedChunk>,
        model_override: Option<CompletionModel>,
    ) -> Result<BoxStream<'static, Result<ChatEvent>>> {
        let context = AgentContext::new(session.clone(), message.to_string(), retrieved_chunks);

        let model = model_override.unwrap_or_else(|| self.config.default_model.clone());

        let executor = AgentExecutor::new(self.config.clone(), self.tools.clone(), context, model);

        executor.run().await
    }

    /// Returns proposed edits from an agent run.
    pub fn extract_edits(&self, _events: &[ChatEvent]) -> Vec<ProposedEdit> {
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
