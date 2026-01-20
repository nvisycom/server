//! Output node types for writing data to storage backends.

use serde::{Deserialize, Serialize};

use super::provider::ProviderParams;

/// A data output node that writes or consumes data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputNode {
    /// Display name of the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this output does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Provider parameters (credentials referenced by ID).
    pub provider: ProviderParams,
}

impl OutputNode {
    /// Creates a new output node.
    pub fn new(provider: ProviderParams) -> Self {
        Self {
            name: None,
            description: None,
            provider,
        }
    }

    /// Sets the display name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl From<ProviderParams> for OutputNode {
    fn from(provider: ProviderParams) -> Self {
        Self::new(provider)
    }
}
