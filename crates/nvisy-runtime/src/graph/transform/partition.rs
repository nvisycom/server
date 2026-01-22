//! Partition transformer.

use nvisy_dal::AnyDataValue;
use serde::{Deserialize, Serialize};

use super::Transform;
use crate::error::Result;
use crate::provider::CredentialsRegistry;

/// Partition transformer for partitioning documents into elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partition {
    /// Partitioning strategy.
    pub strategy: PartitionStrategy,

    /// Whether to include page break markers in output.
    #[serde(default)]
    pub include_page_breaks: bool,

    /// Whether to discard unsupported element types.
    #[serde(default)]
    pub discard_unsupported: bool,
}

impl Transform for Partition {
    async fn transform(
        &self,
        input: Vec<AnyDataValue>,
        _registry: &CredentialsRegistry,
    ) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement document partitioning based on strategy
        // For now, pass through unchanged
        Ok(input)
    }
}

/// Partitioning strategy for document element extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartitionStrategy {
    /// Automatically detect the best partitioning approach.
    #[default]
    Auto,
    /// Fast rule-based partitioning without ML.
    Fast,
    /// Slower ML-based partitioning with layout detection.
    Slow,
    /// Vision-Language Model based partitioning.
    Vlm,
}
