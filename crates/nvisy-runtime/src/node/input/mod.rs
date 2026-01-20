//! Input node types for reading data from storage backends.

mod config;

pub use config::InputConfig;
use serde::{Deserialize, Serialize};

/// A data input node that reads or produces data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputNode {
    /// Display name of the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this input does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Input configuration.
    pub config: InputConfig,
}

impl InputNode {
    /// Creates a new input node.
    pub fn new(config: InputConfig) -> Self {
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

impl From<InputConfig> for InputNode {
    fn from(config: InputConfig) -> Self {
        Self::new(config)
    }
}
