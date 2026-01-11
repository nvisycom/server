//! Prompt building for the agent.

use crate::rag::RetrievedChunk;
use crate::session::{Message, Session};
use crate::tool::ToolDefinition;

/// Builder for constructing agent prompts.
#[derive(Debug, Clone)]
pub struct PromptBuilder {
    system_prompt: String,
    tools: Vec<ToolDefinition>,
    context_chunks: Vec<RetrievedChunk>,
    history: Vec<Message>,
    user_message: String,
}

impl PromptBuilder {
    /// Creates a new prompt builder with the default system prompt.
    pub fn new() -> Self {
        Self {
            system_prompt: default_system_prompt(),
            tools: Vec::new(),
            context_chunks: Vec::new(),
            history: Vec::new(),
            user_message: String::new(),
        }
    }

    /// Sets a custom system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Adds available tools.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    /// Adds retrieved context chunks.
    pub fn with_context(mut self, chunks: Vec<RetrievedChunk>) -> Self {
        self.context_chunks = chunks;
        self
    }

    /// Adds conversation history from session.
    pub fn with_session(mut self, session: &Session) -> Self {
        self.history = session.messages().to_vec();
        if let Some(custom_prompt) = session.system_prompt() {
            self.system_prompt = custom_prompt.to_string();
        }
        self
    }

    /// Sets the user message.
    pub fn with_user_message(mut self, message: impl Into<String>) -> Self {
        self.user_message = message.into();
        self
    }

    /// Builds the system prompt with context.
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = self.system_prompt.clone();

        // Add tool descriptions
        if !self.tools.is_empty() {
            prompt.push_str("\n\n## Available Tools\n\n");
            for tool in &self.tools {
                prompt.push_str(&format!("### {}\n{}\n\n", tool.name(), tool.description()));
            }
        }

        // Add context chunks
        if !self.context_chunks.is_empty() {
            prompt.push_str("\n\n## Document Context\n\n");
            for (i, chunk) in self.context_chunks.iter().enumerate() {
                let content = chunk.content_or_placeholder();
                prompt.push_str(&format!(
                    "### Chunk {} (relevance: {:.2})\n```\n{}\n```\n\n",
                    i + 1,
                    chunk.score,
                    content
                ));
            }
        }

        prompt
    }

    /// Builds the complete message list for the API call.
    pub fn build_messages(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // Add system message
        messages.push(Message::system(self.build_system_prompt()));

        // Add history
        messages.extend(self.history.clone());

        // Add current user message
        if !self.user_message.is_empty() {
            messages.push(Message::user(&self.user_message));
        }

        messages
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Default system prompt for document processing.
fn default_system_prompt() -> String {
    r#"You are an AI assistant specialized in document processing and editing. Your role is to help users understand, analyze, and modify their documents.

## Capabilities

You can:
- Extract specific content (tables, sections, figures)
- Redact sensitive information (PII, confidential data)
- Summarize or restructure content
- Answer questions about the document
- Make precise edits as requested

## Guidelines

1. **Be precise**: When making edits, be specific about locations and changes.
2. **Preserve structure**: Maintain the document's formatting and organization.
3. **Confirm before destructive changes**: For irreversible operations, confirm with the user first.
4. **Reference accurately**: When citing content, use exact quotes or page/section references.
5. **Respect confidentiality**: Handle sensitive content appropriately.

## Tool Usage

Use the available tools to:
- Read document content
- Make edits
- Extract specific elements
- Search within the document

Always explain what you're doing and why."#
        .to_string()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::rag::ChunkMetadata;

    #[test]
    fn prompt_builder_default() {
        let builder = PromptBuilder::new();
        let system = builder.build_system_prompt();

        assert!(system.contains("document processing"));
        assert!(system.contains("Capabilities"));
    }

    #[test]
    fn prompt_builder_with_context() {
        let chunk = RetrievedChunk::new(
            Uuid::nil(),
            Uuid::nil(),
            0.95,
            ChunkMetadata::new(0, 0, 100),
        )
        .with_content("test content".to_string());

        let builder = PromptBuilder::new().with_context(vec![chunk]);

        let system = builder.build_system_prompt();
        assert!(system.contains("Document Context"));
        assert!(system.contains("test content"));
    }
}
