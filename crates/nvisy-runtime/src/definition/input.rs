//! Input node definition types.

use serde::{Deserialize, Serialize};

use super::route::CacheSlot;
use crate::provider::InputProviderParams;

/// Input provider definition for workflow nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputProvider {
    /// Provider parameters (contains credentials_id).
    pub provider: InputProviderParams,
}

/// Input node definition - source of data for the workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Input {
    /// Read from external provider (S3, Postgres, etc.).
    Provider(InputProvider),
    /// Read from named cache slot (resolved at compile time).
    CacheSlot(CacheSlot),
}

impl Input {
    /// Creates a new input from a provider.
    pub fn from_provider(provider: InputProviderParams) -> Self {
        Self::Provider(InputProvider { provider })
    }

    /// Creates a new input from a cache slot.
    pub fn from_cache(slot: impl Into<String>) -> Self {
        Self::CacheSlot(CacheSlot {
            slot: slot.into(),
            priority: None,
        })
    }

    /// Creates a new input from a cache slot with priority.
    pub fn from_cache_with_priority(slot: impl Into<String>, priority: u32) -> Self {
        Self::CacheSlot(CacheSlot {
            slot: slot.into(),
            priority: Some(priority),
        })
    }
}
