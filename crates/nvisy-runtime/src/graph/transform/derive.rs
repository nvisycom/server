//! Derive transformer configuration - generate new content from input.

use serde::{Deserialize, Serialize};

use crate::provider::CompletionProviderParams;

/// Configuration for generating new content from input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeriveConfig {
    /// Completion provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: CompletionProviderParams,

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
