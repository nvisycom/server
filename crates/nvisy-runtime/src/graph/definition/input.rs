//! Input node definition types.

use serde::{Deserialize, Serialize};

use crate::provider::InputProviderParams;

use super::route::CacheSlot;

/// Input provider definition for workflow nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputProvider {
    /// Provider parameters (contains credentials_id).
    pub provider: InputProviderParams,
}

/// Source of input data for an input node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputSource {
    /// Read from external provider (S3, Postgres, etc.).
    Provider(InputProvider),
    /// Read from named cache slot (resolved at compile time).
    CacheSlot(CacheSlot),
}

/// Input node definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputDef {
    /// Source of input data.
    pub source: InputSource,
}

impl InputDef {
    /// Creates a new input definition from a provider.
    pub fn from_provider(provider: InputProviderParams) -> Self {
        Self {
            source: InputSource::Provider(InputProvider { provider }),
        }
    }

    /// Creates a new input definition from a cache slot.
    pub fn from_cache(slot: impl Into<String>) -> Self {
        Self {
            source: InputSource::CacheSlot(CacheSlot {
                slot: slot.into(),
                priority: None,
            }),
        }
    }

    /// Creates a new input definition from a cache slot with priority.
    pub fn from_cache_with_priority(slot: impl Into<String>, priority: u32) -> Self {
        Self {
            source: InputSource::CacheSlot(CacheSlot {
                slot: slot.into(),
                priority: Some(priority),
            }),
        }
    }
}
