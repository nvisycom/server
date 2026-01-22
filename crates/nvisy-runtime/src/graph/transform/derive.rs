//! Derive transformer - generate new content from input.

use nvisy_dal::AnyDataValue;
use serde::{Deserialize, Serialize};

use super::Transform;
use crate::error::Result;
use crate::provider::{CompletionProviderParams, CredentialsRegistry};

/// Derive transformer for generating new content from input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derive {
    /// Completion provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: CompletionProviderParams,

    /// The derivation task to perform.
    pub task: DeriveTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

impl Transform for Derive {
    async fn transform(
        &self,
        input: Vec<AnyDataValue>,
        _registry: &CredentialsRegistry,
    ) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement derivation using completion provider
        // For now, pass through unchanged
        Ok(input)
    }
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
