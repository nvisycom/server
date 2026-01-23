//! Output node definition types.

use serde::{Deserialize, Serialize};

use super::route::CacheSlot;
use crate::provider::OutputProviderConfig;

/// Output provider definition for workflow nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputProvider {
    /// Provider configuration (credentials_id + params).
    #[serde(flatten)]
    pub provider: OutputProviderConfig,
}

/// Output node definition - destination for workflow data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum Output {
    /// Write to external provider (S3, Qdrant, etc.).
    Provider(OutputProvider),
    /// Write to named cache slot (resolved at compile time).
    Cache(CacheSlot),
}

impl Output {
    /// Creates a new output from a provider configuration.
    pub fn from_provider(provider: OutputProviderConfig) -> Self {
        Self::Provider(OutputProvider { provider })
    }

    /// Creates a new output from a cache slot.
    pub fn from_cache(slot: impl Into<String>) -> Self {
        Self::Cache(CacheSlot {
            slot: slot.into(),
            priority: None,
        })
    }

    /// Creates a new output from a cache slot with priority.
    pub fn from_cache_with_priority(slot: impl Into<String>, priority: u32) -> Self {
        Self::Cache(CacheSlot {
            slot: slot.into(),
            priority: Some(priority),
        })
    }
}
