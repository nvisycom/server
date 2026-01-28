//! Derive transform definition.

use nvisy_core::Provider;
use nvisy_rig::provider::{CompletionModel, CompletionProvider, Credentials};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Derive transform for generating new content from input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derive {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,

    /// Completion model to use.
    #[serde(flatten)]
    pub model: CompletionModel,

    /// The derivation task to perform.
    pub task: DeriveTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

impl Derive {
    /// Creates a completion provider from these parameters and credentials.
    pub async fn into_provider(self, credentials: Credentials) -> Result<CompletionProvider> {
        CompletionProvider::connect(self.model, credentials)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
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
