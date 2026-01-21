//! Agent executor that runs the conversation loop.

use std::sync::Arc;

use futures::StreamExt;
use futures::stream::BoxStream;

use super::{AgentConfig, AgentContext, ChatEvent};
use crate::Result;
use crate::provider::CompletionModel;
use crate::tool::ToolRegistry;

/// Executor for running the agent loop.
pub struct AgentExecutor {
    config: AgentConfig,
    tools: Arc<ToolRegistry>,
    context: AgentContext,
    model: CompletionModel,
}

impl AgentExecutor {
    /// Creates a new executor.
    pub fn new(
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        context: AgentContext,
        model: CompletionModel,
    ) -> Self {
        Self {
            config,
            tools,
            context,
            model,
        }
    }

    /// Runs the agent loop and returns a stream of events.
    pub async fn run(self) -> Result<BoxStream<'static, Result<ChatEvent>>> {
        // TODO: Implement the actual agent loop
        let _ = (&self.config, &self.tools, &self.context, &self.model);

        let stream = futures::stream::empty();
        Ok(stream.boxed())
    }
}
