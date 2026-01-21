//! Derive transformer configuration - generate new content from input.

use nvisy_rig::provider::CompletionModel;
use serde::{Deserialize, Serialize};

/// Configuration for generating new content from input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeriveConfig {
    /// Completion model provider configuration.
    #[serde(flatten)]
    pub provider: CompletionModel,

    /// The derivation task to perform.
    pub task: DeriveTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

/// Tasks for generating new content from input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeriveTask {
    /// Generate a condensed summary of the content.
    Summarization,
    /// Generate a title or heading for the content.
    GenerateTitle,
}
