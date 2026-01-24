//! Structured output agent for JSON conversion tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::tool::{JsonResponse, JsonSchemaTool, ScratchpadTool};
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

/// Generic structured output schema for validation.
///
/// This is a flexible schema that accepts any valid JSON structure.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StructuredOutput {
    /// The extracted data as a JSON value.
    #[serde(flatten)]
    pub data: Value,
}

/// Agent for structured output tasks.
///
/// Handles tasks that convert text to structured JSON:
/// - Free-form JSON conversion
/// - Schema-based structured extraction
///
/// When `with_tools` is enabled, the agent has access to:
/// - `ScratchpadTool` - For drafting complex extractions iteratively
/// - `JsonSchemaTool` - For validating output against schemas
pub struct StructuredOutputAgent {
    agent: Agent<CompletionProvider>,
    model_name: String,
}

impl StructuredOutputAgent {
    /// Creates a new structured output agent with the given completion provider.
    ///
    /// # Arguments
    /// * `provider` - The completion provider to use
    /// * `with_tools` - Whether to enable tool usage (scratchpad, schema validation)
    pub fn new(provider: CompletionProvider, with_tools: bool) -> Self {
        let model_name = provider.model_name().to_string();
        let builder = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE);

        let agent = if with_tools {
            builder
                .tool(ScratchpadTool::new())
                .tool(JsonSchemaTool::<StructuredOutput>::new())
                .build()
        } else {
            builder.build()
        };

        Self { agent, model_name }
    }

    /// Converts text to JSON format.
    ///
    /// Attempts to extract structured information from free-form text
    /// and represent it as JSON.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn to_json(&self, text: &str) -> Result<Value> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_TO_JSON, text);
        let response = self.agent.prompt(&prompt).await?;
        let value: Value = JsonResponse::parse(&response)?;
        tracing::debug!("to_json completed");
        Ok(value)
    }

    /// Converts text to JSON matching a specific schema.
    ///
    /// Extracts information from text and structures it according to
    /// the provided JSON schema.
    #[tracing::instrument(skip(self, text, schema), fields(agent = NAME, model = %self.model_name, text_len = text.len(), schema_len = schema.len()))]
    pub async fn to_structured_json(&self, text: &str, schema: &str) -> Result<Value> {
        let base_prompt = PROMPT_TO_STRUCTURED_JSON.replace("{}", schema);
        let prompt = format!("{}\n\nText:\n{}", base_prompt, text);
        let response = self.agent.prompt(&prompt).await?;
        let value: Value = JsonResponse::parse(&response)?;
        tracing::debug!("to_structured_json completed");
        Ok(value)
    }
}
