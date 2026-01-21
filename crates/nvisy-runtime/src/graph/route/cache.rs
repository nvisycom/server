//! Cache slot node type for in-memory data passing.

use serde::{Deserialize, Serialize};

/// A cache slot node that can store and retrieve data within a workflow.
///
/// Cache slots act as named temporary storage that can be used as both
/// input (read from cache) and output (write to cache) within the same workflow.
/// This enables data sharing between different branches of a workflow graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheSlot {
    /// Slot identifier (used as the key for storage/retrieval).
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
