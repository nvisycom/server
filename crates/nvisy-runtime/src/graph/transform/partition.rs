//! Partition transformer configuration.

use serde::{Deserialize, Serialize};

/// Configuration for partitioning data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// Field to partition by.
    pub field: String,
}
