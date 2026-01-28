//! Timing information for operations.
//!
//! This module provides the [`Timing`] struct for capturing start and end
//! timestamps of operations.

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

/// Timing information for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    /// When the operation started.
    pub started_at: Timestamp,
    /// When the operation ended.
    pub ended_at: Timestamp,
}

impl Timing {
    /// Create a new timing with the given start and end timestamps.
    pub fn new(started_at: Timestamp, ended_at: Timestamp) -> Self {
        Self {
            started_at,
            ended_at,
        }
    }

    /// Create a timing from a start timestamp and duration.
    ///
    /// This is useful when you know how long an operation took but need
    /// to construct timing information.
    pub fn from_duration(started_at: Timestamp, duration: SignedDuration) -> Self {
        Self {
            started_at,
            ended_at: started_at + duration,
        }
    }

    /// Create a timing representing an instant (zero duration).
    pub fn instant() -> Self {
        let now = Timestamp::now();
        Self {
            started_at: now,
            ended_at: now,
        }
    }

    /// Get the duration of the operation.
    pub fn duration(&self) -> SignedDuration {
        self.ended_at.duration_since(self.started_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_creation() {
        let start = Timestamp::now();
        let end = start + SignedDuration::from_millis(100);

        let timing = Timing::new(start, end);

        assert_eq!(timing.started_at, start);
        assert_eq!(timing.ended_at, end);
        assert_eq!(timing.duration().as_millis(), 100);
    }

    #[test]
    fn test_timing_from_duration() {
        let start = Timestamp::now();
        let duration = SignedDuration::from_secs(5);

        let timing = Timing::from_duration(start, duration);

        assert_eq!(timing.started_at, start);
        assert_eq!(timing.ended_at, start + duration);
        assert_eq!(timing.duration(), duration);
    }

    #[test]
    fn test_timing_instant() {
        let timing = Timing::instant();

        assert_eq!(timing.duration().as_millis(), 0);
    }
}
