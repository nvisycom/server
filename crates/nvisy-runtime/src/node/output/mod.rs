//! Output node types for writing data to storage backends.

mod config;

pub use config::{OutputConfig, WebhookConfig};
use serde::{Deserialize, Serialize};

/// A data output node that writes or consumes data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputNode {
    /// Display name of the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this output does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Output configuration.
    pub config: OutputConfig,
}

impl OutputNode {
    /// Creates a new output node.
    pub fn new(config: OutputConfig) -> Self {
        Self {
            name: None,
            description: None,
            config,
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

impl From<OutputConfig> for OutputNode {
    fn from(config: OutputConfig) -> Self {
        Self::new(config)
    }
}
