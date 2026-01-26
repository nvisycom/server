//! Vision agent for VLM-powered tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

use super::tool::ScratchpadTool;
use crate::Result;
use crate::provider::CompletionProvider;

const NAME: &str = "VisionAgent";
const DESCRIPTION: &str =
    "Agent for vision-language model tasks including image description, OCR, and object detection";

const PREAMBLE: &str = "\
You are a vision analysis assistant specialized in understanding and describing visual content.
Your task is to analyze images and provide accurate, detailed information based on what you observe.
Always be precise and factual in your descriptions. If you cannot determine something with certainty, say so.
When outputting structured data, use valid JSON format.";

const PROMPT_DESCRIBE: &str = "Describe this image concisely in 1-2 sentences.";

const PROMPT_DESCRIBE_DETAILED: &str = "\
Provide a detailed description of this image, including:
- Main subjects and objects
- Text visible in the image
- Colors and visual style
- Layout and composition";

const PROMPT_EXTRACT_TEXT: &str = "\
Extract all text visible in this image.
Preserve the original formatting and structure as much as possible.
If no text is visible, respond with 'No text detected.'";

const PROMPT_DETECT_OBJECTS: &str = "\
List all objects and entities visible in this image.
For each object, provide:
- Object type/name
- Brief description
- Approximate location (e.g., top-left, center, bottom-right)

Format as a JSON array.";

/// Agent for vision-language model tasks.
///
/// Handles tasks that require understanding visual content:
/// - Image description (brief and detailed)
/// - Generative OCR (text extraction from images)
/// - Object detection
/// - VLM-based document partitioning
///
/// When `with_tools` is enabled, the agent has access to:
/// - `ScratchpadTool` - For drafting and refining descriptions iteratively
pub struct VisionAgent {
    agent: Agent<CompletionProvider>,
    model_name: String,
}

impl VisionAgent {
    /// Creates a new vision agent with the given completion provider.
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

    /// Generates a brief description of an image.
    #[tracing::instrument(skip(self, image_base64), fields(agent = NAME, model = %self.model_name, image_len = image_base64.len()))]
    pub async fn describe(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DESCRIBE, image_base64);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "describe completed");
        Ok(response)
    }

    /// Generates a detailed description of an image.
    #[tracing::instrument(skip(self, image_base64), fields(agent = NAME, model = %self.model_name, image_len = image_base64.len()))]
    pub async fn describe_detailed(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DESCRIBE_DETAILED, image_base64);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "describe_detailed completed");
        Ok(response)
    }

    /// Extracts text from an image using generative OCR.
    #[tracing::instrument(skip(self, image_base64), fields(agent = NAME, model = %self.model_name, image_len = image_base64.len()))]
    pub async fn extract_text(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_EXTRACT_TEXT, image_base64);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "extract_text completed");
        Ok(response)
    }

    /// Detects and lists objects in an image.
    #[tracing::instrument(skip(self, image_base64), fields(agent = NAME, model = %self.model_name, image_len = image_base64.len()))]
    pub async fn detect_objects(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DETECT_OBJECTS, image_base64);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "detect_objects completed");
        Ok(response)
    }
}
