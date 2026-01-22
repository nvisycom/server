//! Output node definition types.

use serde::{Deserialize, Serialize};

use crate::provider::OutputProviderParams;

use super::route::CacheSlot;

/// Output provider definition for workflow nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputProviderDef {
    /// Provider parameters (contains credentials_id).
    pub provider: OutputProviderParams,
}

/// Target destination for an output node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputTarget {
    /// Write to external provider (S3, Qdrant, etc.).
    Provider(OutputProviderDef),
    /// Write to named cache slot (resolved at compile time).
    Cache(CacheSlot),
}

/// Output node definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputDef {
    /// Target destination for output data.
    pub target: OutputTarget,
}

impl OutputDef {
    /// Creates a new output definition from a provider.
    pub fn from_provider(provider: OutputProviderParams) -> Self {
        Self {
            target: OutputTarget::Provider(OutputProviderDef { provider }),
        }
    }

    /// Creates a new output definition from a cache slot.
    pub fn from_cache(slot: impl Into<String>) -> Self {
        Self {
            target: OutputTarget::Cache(CacheSlot {
                slot: slot.into(),
                priority: None,
            }),
        }
    }

    /// Creates a new output definition from a cache slot with priority.
    pub fn from_cache_with_priority(slot: impl Into<String>, priority: u32) -> Self {
        Self {
            target: OutputTarget::Cache(CacheSlot {
                slot: slot.into(),
                priority: Some(priority),
            }),
        }
    }
}
