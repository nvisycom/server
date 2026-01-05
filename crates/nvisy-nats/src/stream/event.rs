//! Event types for stream processing.
//!
//! This module contains priority levels used across all event streams.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Event execution priority levels.
///
/// Priority determines the order in which events are processed when multiple
/// events are queued. Higher priority events are processed before lower priority ones.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum EventPriority {
    /// Low priority - processed when system resources are available.
    Low = 0,

    /// Normal priority - default for most events.
    #[default]
    Normal = 1,

    /// High priority - processed ahead of normal events.
    High = 2,
}

impl EventPriority {
    /// Returns the numeric value of the priority level.
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns true if this is a high priority event.
    #[inline]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(EventPriority::Low < EventPriority::Normal);
        assert!(EventPriority::Normal < EventPriority::High);
    }

    #[test]
    fn test_priority_numeric_values() {
        assert_eq!(EventPriority::Low.as_u8(), 0);
        assert_eq!(EventPriority::Normal.as_u8(), 1);
        assert_eq!(EventPriority::High.as_u8(), 2);
    }

    #[test]
    fn test_priority_default() {
        assert_eq!(EventPriority::default(), EventPriority::Normal);
    }

    #[test]
    fn test_priority_serialization() {
        let priority = EventPriority::High;
        let serialized = serde_json::to_string(&priority).unwrap();
        let deserialized: EventPriority = serde_json::from_str(&serialized).unwrap();
        assert_eq!(priority, deserialized);
    }
}
