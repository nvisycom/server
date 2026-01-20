//! Input node types for reading data from storage backends.

use nvisy_dal::DataTypeId;
use serde::{Deserialize, Serialize};

use super::provider::ProviderParams;

/// A data input node that reads or produces data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputNode {
    /// Display name of the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this input does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Provider parameters (credentials referenced by ID).
    pub provider: ProviderParams,
}

impl InputNode {
    /// Creates a new input node.
    pub fn new(provider: ProviderParams) -> Self {
        Self {
            name: None,
            description: None,
            provider,
        }
    }

    /// Returns the output data type based on the provider kind.
    pub const fn output_type(&self) -> DataTypeId {
        self.provider.output_type()
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

impl From<ProviderParams> for InputNode {
    fn from(provider: ProviderParams) -> Self {
        Self::new(provider)
    }
}
