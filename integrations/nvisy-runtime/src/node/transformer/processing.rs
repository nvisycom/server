//! Data processing transformer configurations.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

/// Configuration for content chunking (simple character-based).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(
    name = "ChunkContentConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct ChunkContentConfig {
    /// Maximum chunk size in characters.
    pub max_chunk_size: usize,
    /// Overlap between chunks in characters.
    #[serde(default)]
    #[builder(default)]
    pub overlap: usize,
}

impl ChunkContentConfigBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.max_chunk_size.is_some_and(|s| s == 0) {
            return Err("max_chunk_size must be greater than 0".into());
        }
        if let (Some(max), Some(overlap)) = (&self.max_chunk_size, &self.overlap)
            && overlap >= max
        {
            return Err("overlap must be less than max_chunk_size".into());
        }
        Ok(())
    }
}

/// Configuration for LLM transformation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(
    name = "LlmTransformConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct LlmTransformConfig {
    /// Model identifier.
    pub model: String,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub system_prompt: Option<String>,
    /// User prompt template.
    pub prompt_template: String,
    /// Temperature for generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub max_tokens: Option<usize>,
}

impl LlmTransformConfigBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.model.as_ref().is_some_and(|m| m.is_empty()) {
            return Err("model cannot be empty".into());
        }
        if self.prompt_template.as_ref().is_some_and(|p| p.is_empty()) {
            return Err("prompt_template cannot be empty".into());
        }
        if let Some(Some(temp)) = &self.temperature
            && (*temp < 0.0 || *temp > 2.0)
        {
            return Err("temperature must be between 0.0 and 2.0".into());
        }
        Ok(())
    }
}

/// Configuration for format conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvertFormatConfig {
    /// Target format.
    pub target_format: String,
    /// Format-specific options.
    #[serde(default)]
    pub options: serde_json::Value,
}

/// Configuration for validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateConfig {
    /// JSON schema for validation.
    pub schema: serde_json::Value,
    /// Whether to fail on validation error.
    #[serde(default = "default_true")]
    pub fail_on_error: bool,
}

/// Configuration for filtering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Filter expression.
    pub expression: String,
}

/// Configuration for merging.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MergeConfig {
    /// Merge strategy.
    #[serde(default)]
    pub strategy: MergeStrategy,
}

/// Merge strategy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Concatenate all inputs.
    #[default]
    Concatenate,
    /// Interleave inputs.
    Interleave,
    /// Take first non-empty input.
    First,
}

fn default_true() -> bool {
    true
}
