//! Output node types for writing data to storage backends and vector databases.

use nvisy_dal::DataTypeId;
use serde::{Deserialize, Serialize};

use crate::provider::OutputProviderParams;

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
    pub provider: OutputProviderParams,
}

impl OutputNode {
    /// Creates a new output node.
    pub fn new(provider: OutputProviderParams) -> Self {
        Self {
            name: None,
            description: None,
            provider,
        }
    }

    /// Returns the expected input data type based on the provider kind.
    pub const fn input_type(&self) -> DataTypeId {
        self.provider.output_type()
    }
}

impl From<OutputProviderParams> for OutputNode {
    fn from(provider: OutputProviderParams) -> Self {
        Self::new(provider)
    }
}
