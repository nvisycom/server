//! Edit module for document modifications.
//!
//! This module handles:
//! - Proposed edits from the agent
//! - Edit approval and rejection
//! - Edit application to documents
//! - Edit preview generation

mod operation;
mod proposed;

pub use operation::{EditLocation, EditOperation};
pub use proposed::ProposedEdit;
use uuid::Uuid;

/// Result of applying edits.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Successfully applied edits.
    pub applied: Vec<ProposedEdit>,

    /// Edits that were not found.
    pub not_found: Vec<Uuid>,

    /// Edits that failed to apply.
    pub errors: Vec<ApplyError>,
}

impl ApplyResult {
    /// Returns whether all edits were applied successfully.
    pub fn is_success(&self) -> bool {
        self.not_found.is_empty() && self.errors.is_empty()
    }

    /// Returns the count of successfully applied edits.
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    /// Returns the count of failed edits.
    pub fn failed_count(&self) -> usize {
        self.not_found.len() + self.errors.len()
    }
}

/// Error that occurred while applying an edit.
#[derive(Debug, Clone)]
pub struct ApplyError {
    /// The edit ID that failed.
    pub edit_id: Uuid,

    /// Error message.
    pub message: String,

    /// Whether the error is recoverable.
    pub recoverable: bool,
}

impl ApplyError {
    /// Creates a new apply error.
    pub fn new(edit_id: Uuid, message: impl Into<String>) -> Self {
        Self {
            edit_id,
            message: message.into(),
            recoverable: false,
        }
    }

    /// Marks the error as recoverable.
    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_result_success() {
        let result = ApplyResult {
            applied: vec![],
            not_found: vec![],
            errors: vec![],
        };

        assert!(result.is_success());
    }

    #[test]
    fn apply_result_with_errors() {
        let result = ApplyResult {
            applied: vec![],
            not_found: vec![Uuid::now_v7()],
            errors: vec![],
        };

        assert!(!result.is_success());
        assert_eq!(result.failed_count(), 1);
    }
}
