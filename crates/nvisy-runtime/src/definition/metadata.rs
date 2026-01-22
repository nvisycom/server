//! Workflow metadata.

use derive_builder::Builder;
use jiff::Timestamp;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Workflow metadata.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Builder)]
#[builder(
    name = "WorkflowMetadataBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct WorkflowMetadata {
    /// Workflow name (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub name: Option<String>,
    /// Workflow description.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub description: Option<String>,
    /// Workflow version (semver, optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub version: Option<Version>,
    /// Tags for organization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub tags: Vec<String>,
    /// Creation timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub created_at: Option<Timestamp>,
    /// Last update timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub updated_at: Option<Timestamp>,
}

impl WorkflowMetadataBuilder {
    fn validate(&self) -> Result<(), String> {
        // All fields are optional, so validation always succeeds
        Ok(())
    }
}

impl WorkflowMetadata {
    /// Creates a new empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a builder for creating workflow metadata.
    pub fn builder() -> WorkflowMetadataBuilder {
        WorkflowMetadataBuilder::default()
    }
}
