//! Structured output agent for JSON conversion tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

use crate::Result;
use crate::provider::CompletionProvider;

const NAME: &str = "StructuredOutputAgent";
const DESCRIPTION: &str =
    "Agent for converting unstructured text to structured JSON with optional schema validation";

const PREAMBLE: &str = "\
You are a data extraction assistant specialized in converting unstructured text to structured JSON.
Your task is to identify and extract relevant information and format it as valid JSON.
When a schema is provided, strictly adhere to it. Use null for fields that cannot be determined.
Always output valid JSON, no explanations or markdown formatting.";

const PROMPT_TO_JSON: &str = "\
Convert the following text to a well-structured JSON object.
Identify the key information and organize it logically.
Only output valid JSON, no explanation.";

const PROMPT_TO_STRUCTURED_JSON: &str = "\
Extract information from the following text and format it as JSON matching this schema:

Schema:
{}

Only output valid JSON that conforms to the schema, no explanation.
If a field cannot be determined from the text, use null.";

/// Agent for structured output tasks.
///
/// Handles tasks that convert text to structured JSON:
/// - Free-form JSON conversion
/// - Schema-based structured extraction
pub struct StructuredOutputAgent {
    agent: Agent<CompletionProvider>,
}

impl StructuredOutputAgent {
    /// Creates a new structured output agent with the given completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        let agent = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .build();
        Self { agent }
    }

    /// Converts text to JSON format.
    ///
    /// Attempts to extract structured information from free-form text
    /// and represent it as JSON.
    pub async fn to_json(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_TO_JSON, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Converts text to JSON matching a specific schema.
    ///
    /// Extracts information from text and structures it according to
    /// the provided JSON schema.
    pub async fn to_structured_json(&self, text: &str, schema: &str) -> Result<String> {
        let base_prompt = PROMPT_TO_STRUCTURED_JSON.replace("{}", schema);
        let prompt = format!("{}\n\nText:\n{}", base_prompt, text);
        Ok(self.agent.prompt(&prompt).await?)
    }
}
