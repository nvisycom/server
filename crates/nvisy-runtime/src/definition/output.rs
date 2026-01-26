//! Output node definition types.

use serde::{Deserialize, Serialize};

use super::route::CacheSlot;

/// Output node definition - destination for workflow data.
///
/// Storage provider outputs (S3, Qdrant, etc.) are handled externally via Python.
/// This enum only supports cache slots for internal workflow data flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum Output {
    /// Write to named cache slot (resolved at compile time).
    Cache(CacheSlot),
}

impl Output {
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
