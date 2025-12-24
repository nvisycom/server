//! Document status enumeration for document lifecycle management.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the current status of a document in its lifecycle.
///
/// This enumeration corresponds to the `DOCUMENT_STATUS` PostgreSQL enum and is used
/// to track document states from creation through processing, completion, and archival.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::DocumentStatus"]
pub enum DocumentStatus {
    /// Document is being created or edited (work in progress)
    #[db_rename = "draft"]
    #[serde(rename = "draft")]
    #[default]
    Draft,

    /// Document is currently being processed by the system
    #[db_rename = "processing"]
    #[serde(rename = "processing")]
    Processing,

    /// Document is ready for use and fully processed
    #[db_rename = "ready"]
    #[serde(rename = "ready")]
    Ready,

    /// Document is archived but remains accessible
    #[db_rename = "archived"]
    #[serde(rename = "archived")]
    Archived,

    /// Document is locked for editing (read-only)
    #[db_rename = "locked"]
    #[serde(rename = "locked")]
    Locked,

    /// Document processing failed or encountered an error
    #[db_rename = "error"]
    #[serde(rename = "error")]
    Error,
}

impl DocumentStatus {
    /// Returns whether the document can be edited.
    #[inline]
    pub fn is_editable(self) -> bool {
        matches!(self, DocumentStatus::Draft)
    }

    /// Returns whether the document is read-only.
    #[inline]
    pub fn is_read_only(self) -> bool {
        matches!(self, DocumentStatus::Archived | DocumentStatus::Locked)
    }

    /// Returns whether the document is currently being processed.
    #[inline]
    pub fn is_processing(self) -> bool {
        matches!(self, DocumentStatus::Processing)
    }

    /// Returns whether the document is available for normal use.
    #[inline]
    pub fn is_available(self) -> bool {
        matches!(
            self,
            DocumentStatus::Ready | DocumentStatus::Archived | DocumentStatus::Locked
        )
    }

    /// Returns whether the document is in a completed state.
    #[inline]
    pub fn is_completed(self) -> bool {
        matches!(self, DocumentStatus::Ready | DocumentStatus::Archived)
    }

    /// Returns whether the document is in a draft state.
    #[inline]
    pub fn is_draft(self) -> bool {
        matches!(self, DocumentStatus::Draft)
    }

    /// Returns whether the document is archived.
    #[inline]
    pub fn is_archived(self) -> bool {
        matches!(self, DocumentStatus::Archived)
    }

    /// Returns whether the document has encountered an error.
    #[inline]
    pub fn has_error(self) -> bool {
        matches!(self, DocumentStatus::Error)
    }

    /// Returns whether the document is locked.
    #[inline]
    pub fn is_locked(self) -> bool {
        matches!(self, DocumentStatus::Locked)
    }

    /// Returns whether the document can be processed.
    #[inline]
    pub fn can_be_processed(self) -> bool {
        matches!(self, DocumentStatus::Draft | DocumentStatus::Error)
    }

    /// Returns whether the document can be unlocked.
    #[inline]
    pub fn can_be_unlocked(self) -> bool {
        matches!(self, DocumentStatus::Locked)
    }

    /// Returns whether the document can be archived.
    #[inline]
    pub fn can_be_archived(self) -> bool {
        matches!(self, DocumentStatus::Ready | DocumentStatus::Locked)
    }

    /// Returns whether the document can be restored from archive.
    #[inline]
    pub fn can_be_restored(self) -> bool {
        matches!(self, DocumentStatus::Archived)
    }

    /// Returns whether files can be added to this document.
    #[inline]
    pub fn allows_file_uploads(self) -> bool {
        matches!(self, DocumentStatus::Draft)
    }

    /// Returns whether the document status indicates a stable state.
    #[inline]
    pub fn is_stable(self) -> bool {
        matches!(
            self,
            DocumentStatus::Ready | DocumentStatus::Archived | DocumentStatus::Locked
        )
    }

    /// Returns document statuses that are considered active (not archived or error).
    pub fn active_statuses() -> &'static [DocumentStatus] {
        &[
            DocumentStatus::Draft,
            DocumentStatus::Processing,
            DocumentStatus::Ready,
            DocumentStatus::Locked,
        ]
    }
}
