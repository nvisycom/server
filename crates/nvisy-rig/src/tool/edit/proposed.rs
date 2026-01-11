//! Proposed edit types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{EditLocation, EditOperation};

/// A proposed edit to a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedEdit {
    /// Unique edit ID.
    id: Uuid,

    /// Document being edited.
    document_id: Uuid,

    /// Type of operation.
    operation: EditOperation,

    /// Location of the edit.
    location: EditLocation,

    /// Original content (for replace/delete).
    original: Option<String>,

    /// New content (for replace/insert).
    replacement: Option<String>,

    /// Reason for the edit.
    reason: String,

    /// Preview of the result.
    preview: Option<String>,

    /// When the edit was proposed.
    proposed_at: DateTime<Utc>,

    /// Current status.
    status: EditStatus,
}

impl ProposedEdit {
    /// Creates a new proposed edit.
    pub fn new(
        document_id: Uuid,
        operation: EditOperation,
        location: EditLocation,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            document_id,
            operation,
            location,
            original: None,
            replacement: None,
            reason: reason.into(),
            preview: None,
            proposed_at: Utc::now(),
            status: EditStatus::Pending,
        }
    }

    /// Creates a replace edit.
    pub fn replace(
        document_id: Uuid,
        location: EditLocation,
        original: impl Into<String>,
        replacement: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            document_id,
            operation: EditOperation::Replace,
            location,
            original: Some(original.into()),
            replacement: Some(replacement.into()),
            reason: reason.into(),
            preview: None,
            proposed_at: Utc::now(),
            status: EditStatus::Pending,
        }
    }

    /// Creates an insert edit.
    pub fn insert(
        document_id: Uuid,
        location: EditLocation,
        content: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            document_id,
            operation: EditOperation::Insert,
            location,
            original: None,
            replacement: Some(content.into()),
            reason: reason.into(),
            preview: None,
            proposed_at: Utc::now(),
            status: EditStatus::Pending,
        }
    }

    /// Creates a delete edit.
    pub fn delete(
        document_id: Uuid,
        location: EditLocation,
        content: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            document_id,
            operation: EditOperation::Delete,
            location,
            original: Some(content.into()),
            replacement: None,
            reason: reason.into(),
            preview: None,
            proposed_at: Utc::now(),
            status: EditStatus::Pending,
        }
    }

    /// Creates a redact edit.
    pub fn redact(
        document_id: Uuid,
        location: EditLocation,
        content: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            document_id,
            operation: EditOperation::Redact,
            location,
            original: Some(content.into()),
            replacement: Some("[REDACTED]".to_string()),
            reason: reason.into(),
            preview: None,
            proposed_at: Utc::now(),
            status: EditStatus::Pending,
        }
    }

    /// Adds a preview.
    pub fn with_preview(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }

    /// Returns the edit ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the document ID.
    pub fn document_id(&self) -> Uuid {
        self.document_id
    }

    /// Returns the operation type.
    pub fn operation(&self) -> EditOperation {
        self.operation
    }

    /// Returns the operation type as a string.
    pub fn operation_type(&self) -> &'static str {
        self.operation.name()
    }

    /// Returns the location.
    pub fn location(&self) -> &EditLocation {
        &self.location
    }

    /// Returns the original content.
    pub fn original(&self) -> Option<&str> {
        self.original.as_deref()
    }

    /// Returns the replacement content.
    pub fn replacement(&self) -> Option<&str> {
        self.replacement.as_deref()
    }

    /// Returns the reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Returns the preview.
    pub fn preview(&self) -> Option<&str> {
        self.preview.as_deref()
    }

    /// Returns when the edit was proposed.
    pub fn proposed_at(&self) -> DateTime<Utc> {
        self.proposed_at
    }

    /// Returns the current status.
    pub fn status(&self) -> EditStatus {
        self.status
    }

    /// Returns whether this operation is idempotent.
    pub fn is_idempotent(&self) -> bool {
        self.operation.is_idempotent()
    }

    /// Returns whether this edit is pending.
    pub fn is_pending(&self) -> bool {
        self.status == EditStatus::Pending
    }

    /// Marks the edit as applied.
    pub fn mark_applied(&mut self) {
        self.status = EditStatus::Applied;
    }

    /// Marks the edit as rejected.
    pub fn mark_rejected(&mut self) {
        self.status = EditStatus::Rejected;
    }

    /// Returns a summary of the edit for display.
    pub fn summary(&self) -> String {
        format!(
            "{} at {}: {}",
            self.operation.name(),
            self.location.display(),
            self.reason
        )
    }
}

/// Status of a proposed edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditStatus {
    /// Awaiting user approval.
    Pending,

    /// Approved and applied.
    Applied,

    /// Rejected by user.
    Rejected,

    /// Failed to apply.
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_edit_replace() {
        let edit = ProposedEdit::replace(
            Uuid::now_v7(),
            EditLocation::page(1),
            "old text",
            "new text",
            "fixing typo",
        );

        assert_eq!(edit.operation(), EditOperation::Replace);
        assert_eq!(edit.original(), Some("old text"));
        assert_eq!(edit.replacement(), Some("new text"));
        assert!(edit.is_pending());
    }

    #[test]
    fn proposed_edit_redact() {
        let edit = ProposedEdit::redact(
            Uuid::now_v7(),
            EditLocation::page(5),
            "SSN: 123-45-6789",
            "removing PII",
        );

        assert_eq!(edit.operation(), EditOperation::Redact);
        assert_eq!(edit.replacement(), Some("[REDACTED]"));
    }

    #[test]
    fn proposed_edit_summary() {
        let edit = ProposedEdit::delete(
            Uuid::now_v7(),
            EditLocation::section("Appendix"),
            "old content",
            "removing outdated section",
        );

        let summary = edit.summary();
        assert!(summary.contains("delete"));
        assert!(summary.contains("Appendix"));
    }
}
