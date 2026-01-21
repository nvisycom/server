//! Partition transformer configuration.

use serde::{Deserialize, Serialize};

/// Configuration for partitioning documents into elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// Partitioning strategy.
    pub strategy: PartitionStrategy,

    /// Whether to include page break markers in output.
    #[serde(default)]
    pub include_page_breaks: bool,

    /// Whether to discard unsupported element types.
    #[serde(default)]
    pub discard_unsupported: bool,
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
