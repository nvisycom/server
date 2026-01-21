//! Text generation agent for creating new text content.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

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
pub struct TextGenerationAgent {
    agent: Agent<CompletionProvider>,
}

impl TextGenerationAgent {
    /// Creates a new text generation agent with the given completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        let agent = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .build();
        Self { agent }
    }

    /// Generates a summary of the text.
    pub async fn summarize(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_SUMMARIZE, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Generates a title for the text.
    pub async fn generate_title(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_GENERATE_TITLE, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Generates contextual information for a chunk.
    ///
    /// This is used for contextual chunking, where each chunk is enriched
    /// with context about how it fits into the larger document.
    pub async fn generate_chunk_context(
        &self,
        chunk: &str,
        document_summary: &str,
    ) -> Result<String> {
        let prompt = format!(
            "{}\n\nDocument Summary:\n{}\n\nChunk:\n{}",
            PROMPT_GENERATE_CHUNK_CONTEXT, document_summary, chunk
        );
        Ok(self.agent.prompt(&prompt).await?)
    }
}
