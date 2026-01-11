//! Agent executor that runs the conversation loop.

use std::sync::Arc;

use futures::StreamExt;
use futures::stream::BoxStream;

use super::{AgentConfig, AgentContext, ChatEvent};
use crate::Result;
use crate::provider::{ModelRef, ProviderRegistry};
use crate::tool::ToolRegistry;

/// Executor for running the agent loop.
pub struct AgentExecutor {
    config: AgentConfig,
    providers: Arc<ProviderRegistry>,
    tools: Arc<ToolRegistry>,
    context: AgentContext,
    model_override: Option<ModelRef>,
}

impl AgentExecutor {
    /// Creates a new executor.
    pub fn new(
        config: AgentConfig,
        providers: Arc<ProviderRegistry>,
        tools: Arc<ToolRegistry>,
        context: AgentContext,
        model_override: Option<ModelRef>,
    ) -> Self {
        Self {
            config,
            providers,
            tools,
            context,
            model_override,
        }
    }

    /// Runs the agent loop and returns a stream of events.
    pub async fn run(self) -> Result<BoxStream<'static, Result<ChatEvent>>> {
        // TODO: Implement the actual agent loop:
        // 1. Build the prompt with system message, context, and history
        // 2. Stream completion from the provider
        // 3. Parse tool calls from the response
        // 4. Execute tools and collect results
        // 5. If tools were called, loop back to step 2
        // 6. Extract proposed edits from tool results
        // 7. Apply auto-apply policies
        // 8. Emit final Done event

        let _ = (
            &self.config,
            &self.providers,
            &self.tools,
            &self.context,
            &self.model_override,
        );

        // For now, return an empty stream
        let stream = futures::stream::empty();
        Ok(stream.boxed())
    }
}
