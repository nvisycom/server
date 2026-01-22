//! Image analysis tool using VLM.

use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::agent::VisionAgent;

/// Error type for image analysis operations.
#[derive(Debug, thiserror::Error)]
pub enum ImageAnalysisError {
    #[error("analysis failed: {0}")]
    Analysis(String),
    #[error("invalid image: {0}")]
    InvalidImage(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
}

impl From<crate::Error> for ImageAnalysisError {
    fn from(e: crate::Error) -> Self {
        Self::Analysis(e.to_string())
    }
}

/// The type of analysis to perform.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisType {
    /// Brief description (1-2 sentences).
    Describe,
    /// Detailed description.
    DescribeDetailed,
    /// Extract text (OCR).
    ExtractText,
    /// Detect objects.
    DetectObjects,
    /// Custom prompt.
    Custom { prompt: String },
}

/// Arguments for image analysis.
#[derive(Debug, Deserialize)]
pub struct ImageAnalysisArgs {
    /// The image data as base64 or URL.
    pub image: String,
    /// The type of analysis to perform.
    #[serde(default = "default_analysis_type")]
    pub analysis_type: AnalysisType,
}

fn default_analysis_type() -> AnalysisType {
    AnalysisType::Describe
}

/// Result of image analysis.
#[derive(Debug, Serialize)]
pub struct ImageAnalysisResult {
    /// The analysis result.
    pub result: String,
    /// The type of analysis performed.
    pub analysis_type: AnalysisType,
}

/// Tool for analyzing images using VLM.
pub struct ImageAnalysisTool {
    agent: Arc<VisionAgent>,
}

impl ImageAnalysisTool {
    /// Creates a new image analysis tool.
    pub fn new(agent: VisionAgent) -> Self {
        Self {
            agent: Arc::new(agent),
        }
    }

    /// Creates a new image analysis tool from an Arc.
    pub fn from_arc(agent: Arc<VisionAgent>) -> Self {
        Self { agent }
    }
}

impl Tool for ImageAnalysisTool {
    type Args = ImageAnalysisArgs;
    type Error = ImageAnalysisError;
    type Output = ImageAnalysisResult;

    const NAME: &'static str = "image_analysis";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Analyze an image using vision-language model. Can describe images, extract text (OCR), detect objects, or answer custom questions about the image.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "image": {
                        "type": "string",
                        "description": "The image as base64-encoded data or a URL"
                    },
                    "analysis_type": {
                        "type": "object",
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "describe": { "type": "object" }
                                },
                                "description": "Brief description (1-2 sentences)"
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "describe_detailed": { "type": "object" }
                                },
                                "description": "Detailed description"
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "extract_text": { "type": "object" }
                                },
                                "description": "Extract text from the image (OCR)"
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "detect_objects": { "type": "object" }
                                },
                                "description": "Detect and list objects in the image"
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "custom": {
                                        "type": "object",
                                        "properties": {
                                            "prompt": { "type": "string" }
                                        },
                                        "required": ["prompt"]
                                    }
                                },
                                "description": "Custom analysis with your own prompt"
                            }
                        ],
                        "description": "The type of analysis to perform (default: describe)"
                    }
                },
                "required": ["image"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = match &args.analysis_type {
            AnalysisType::Describe => self.agent.describe(&args.image).await?,
            AnalysisType::DescribeDetailed => self.agent.describe_detailed(&args.image).await?,
            AnalysisType::ExtractText => self.agent.extract_text(&args.image).await?,
            AnalysisType::DetectObjects => self.agent.detect_objects(&args.image).await?,
            AnalysisType::Custom { prompt } => {
                // For custom prompts, we use describe with a modified prompt
                // In a real implementation, VisionAgent would have a custom method
                let custom_prompt = format!("{}\n\n[Image: {}]", prompt, args.image);
                self.agent.describe(&custom_prompt).await?
            }
        };

        Ok(ImageAnalysisResult {
            result,
            analysis_type: args.analysis_type,
        })
    }
}
