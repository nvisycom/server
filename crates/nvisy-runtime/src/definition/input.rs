//! Input node definition types.

use serde::{Deserialize, Serialize};

use super::route::CacheSlot;

/// Input node definition - source of data for the workflow.
///
/// Storage provider inputs (S3, Postgres, etc.) are handled externally via Python.
/// This enum only supports cache slots for internal workflow data flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Input {
    /// Read from named cache slot (resolved at compile time).
    CacheSlot(CacheSlot),
}

impl Input {
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
