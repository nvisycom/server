//! Cache slot types for in-memory data passing.

use serde::{Deserialize, Serialize};

/// A cache slot reference for in-memory data passing.
///
/// Cache slots act as named connection points that link different parts
/// of a workflow graph. During compilation, cache slots are resolved by
/// connecting incoming edges directly to outgoing edges with matching slot names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSlot {
    /// Slot identifier (used as the key for matching inputs to outputs).
    pub slot: String,
    /// Priority for ordering when multiple slots are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
}

impl CacheSlot {
    /// Creates a new cache slot with the given slot name.
    pub fn new(slot: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            priority: None,
        }
    }

    /// Sets the priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = Some(priority);
        self
    }
}
