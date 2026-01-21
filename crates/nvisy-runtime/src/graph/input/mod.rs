//! Input node types for reading data from storage backends.

use nvisy_dal::DataTypeId;
use serde::{Deserialize, Serialize};

use crate::provider::InputProviderParams;

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
    pub provider: InputProviderParams,
}

impl InputNode {
    /// Creates a new input node.
    pub fn new(provider: InputProviderParams) -> Self {
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
}

impl From<InputProviderParams> for InputNode {
    fn from(provider: InputProviderParams) -> Self {
        Self::new(provider)
    }
}
