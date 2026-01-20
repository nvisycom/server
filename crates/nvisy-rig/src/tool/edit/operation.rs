//! Edit operations and locations.

use serde::{Deserialize, Serialize};

/// Location within a document for an edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditLocation {
    /// Page number (1-indexed).
    pub page: Option<u32>,

    /// Section identifier.
    pub section: Option<String>,

    /// Character offset from start.
    pub offset: Option<usize>,

    /// Length of the affected region.
    pub length: Option<usize>,
}

impl EditLocation {
    /// Creates a page-based location.
    pub fn page(page: u32) -> Self {
        Self {
            page: Some(page),
            section: None,
            offset: None,
            length: None,
        }
    }

    /// Creates a section-based location.
    pub fn section(section: impl Into<String>) -> Self {
        Self {
            page: None,
            section: Some(section.into()),
            offset: None,
            length: None,
        }
    }

    /// Creates an offset-based location.
    pub fn offset(offset: usize, length: usize) -> Self {
        Self {
            page: None,
            section: None,
            offset: Some(offset),
            length: Some(length),
        }
    }

    /// Adds page information.
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Adds section information.
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// Returns a display string for the location.
    pub fn display(&self) -> String {
        let mut parts = Vec::new();

        if let Some(page) = self.page {
            parts.push(format!("page {page}"));
        }

        if let Some(section) = &self.section {
            parts.push(format!("'{section}'"));
        }

        if let Some(offset) = self.offset {
            if let Some(length) = self.length {
                parts.push(format!("offset {offset}..{}", offset + length));
            } else {
                parts.push(format!("offset {offset}"));
            }
        }

        if parts.is_empty() {
            "unspecified location".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Type of edit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOperation {
    /// Replace existing content.
    Replace,

    /// Insert new content.
    Insert,

    /// Delete content.
    Delete,

    /// Redact content (replace with placeholder).
    Redact,

    /// Extract content (copy without modifying).
    Extract,
}

impl EditOperation {
    /// Returns whether this operation is idempotent.
    pub fn is_idempotent(&self) -> bool {
        matches!(self, Self::Extract)
    }

    /// Returns whether this operation modifies the document.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Replace | Self::Insert | Self::Delete | Self::Redact
        )
    }

    /// Returns a human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Insert => "insert",
            Self::Delete => "delete",
            Self::Redact => "redact",
            Self::Extract => "extract",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_location_display() {
        let loc = EditLocation::page(5).with_section("Introduction");
        let display = loc.display();

        assert!(display.contains("page 5"));
        assert!(display.contains("Introduction"));
    }

    #[test]
    fn edit_operation_idempotency() {
        assert!(EditOperation::Extract.is_idempotent());
        assert!(!EditOperation::Replace.is_idempotent());
        assert!(!EditOperation::Delete.is_idempotent());
    }

    #[test]
    fn edit_operation_destructive() {
        assert!(EditOperation::Replace.is_destructive());
        assert!(EditOperation::Delete.is_destructive());
        assert!(!EditOperation::Extract.is_destructive());
    }
}
