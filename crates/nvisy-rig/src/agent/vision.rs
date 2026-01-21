//! Vision agent for VLM-powered tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

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
pub struct VisionAgent {
    agent: Agent<CompletionProvider>,
}

impl VisionAgent {
    /// Creates a new vision agent with the given completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        let agent = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .build();
        Self { agent }
    }

    /// Generates a brief description of an image.
    pub async fn describe(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DESCRIBE, image_base64);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Generates a detailed description of an image.
    pub async fn describe_detailed(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DESCRIBE_DETAILED, image_base64);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Extracts text from an image using generative OCR.
    pub async fn extract_text(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_EXTRACT_TEXT, image_base64);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Detects and lists objects in an image.
    pub async fn detect_objects(&self, image_base64: &str) -> Result<String> {
        let prompt = format!("{}\n\n[Image: {}]", PROMPT_DETECT_OBJECTS, image_base64);
        Ok(self.agent.prompt(&prompt).await?)
    }
}
