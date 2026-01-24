//! Text generation agent for creating new text content.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

use super::tool::ScratchpadTool;
use crate::Result;
use crate::provider::CompletionProvider;

const NAME: &str = "TextGenerationAgent";
const DESCRIPTION: &str =
    "Agent for text generation including summarization, title generation, and contextual chunking";

const PREAMBLE: &str = "\
You are a text generation assistant specialized in creating concise, high-quality content.
Your task is to generate summaries, titles, and contextual information based on input text.
Maintain accuracy while being concise. Preserve the key information and main points.";

const PROMPT_SUMMARIZE: &str = "\
Summarize the following text concisely while preserving the key information and main points.
The summary should be about 20-30% of the original length.";

const PROMPT_GENERATE_TITLE: &str = "\
Generate a concise, descriptive title for the following text.
The title should capture the main topic and be no more than 10 words.

Only output the title, no explanation or quotes.";

const PROMPT_GENERATE_CHUNK_CONTEXT: &str = "\
Given the following document summary and a specific chunk from that document, \
generate a brief context statement (1-2 sentences) that situates this chunk \
within the broader document. This context will be prepended to the chunk \
to improve retrieval quality.

Only output the context statement, no explanation.";

/// Agent for text generation tasks.
///
/// Handles tasks that generate new text content:
/// - Summarization
/// - Title generation
/// - Contextual chunking (adding context to chunks)
///
/// When `with_tools` is enabled, the agent has access to:
/// - `ScratchpadTool` - For drafting and refining content iteratively
pub struct TextGenerationAgent {
    agent: Agent<CompletionProvider>,
    model_name: String,
}

impl TextGenerationAgent {
    /// Creates a new text generation agent with the given completion provider.
    ///
    /// # Arguments
    /// * `provider` - The completion provider to use
    /// * `with_tools` - Whether to enable tool usage (scratchpad for drafting)
    pub fn new(provider: CompletionProvider, with_tools: bool) -> Self {
        let model_name = provider.model_name().to_string();
        let builder = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE);

        let agent = if with_tools {
            builder.tool(ScratchpadTool::new()).build()
        } else {
            builder.build()
        };

        Self { agent, model_name }
    }

    /// Generates a summary of the text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn summarize(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_SUMMARIZE, text);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "summarize completed");
        Ok(response)
    }

    /// Generates a title for the text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn generate_title(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_GENERATE_TITLE, text);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(title = %response, "generate_title completed");
        Ok(response)
    }

    /// Generates contextual information for a chunk.
    ///
    /// This is used for contextual chunking, where each chunk is enriched
    /// with context about how it fits into the larger document.
    #[tracing::instrument(skip(self, chunk, document_summary), fields(agent = NAME, model = %self.model_name, chunk_len = chunk.len(), summary_len = document_summary.len()))]
    pub async fn generate_chunk_context(
        &self,
        chunk: &str,
        document_summary: &str,
    ) -> Result<String> {
        let prompt = format!(
            "{}\n\nDocument Summary:\n{}\n\nChunk:\n{}",
            PROMPT_GENERATE_CHUNK_CONTEXT, document_summary, chunk
        );
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(
            response_len = response.len(),
            "generate_chunk_context completed"
        );
        Ok(response)
    }
}
