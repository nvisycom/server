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
}

impl DocumentStatus {
    /// Returns whether the document is currently being processed.
    #[inline]
    pub fn is_processing(self) -> bool {
        matches!(self, DocumentStatus::Processing)
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

    /// Returns whether the document is ready for use.
    #[inline]
    pub fn is_ready(self) -> bool {
        matches!(self, DocumentStatus::Ready)
    }
}
