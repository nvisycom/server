//! Annotation type enumeration for document annotations.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of annotation for document content.
///
/// This enumeration corresponds to the `ANNOTATION_TYPE` PostgreSQL enum and is used
/// to classify different types of annotations users can create on documents.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::AnnotationType"]
pub enum AnnotationType {
    /// General text note or annotation
    #[db_rename = "note"]
    #[serde(rename = "note")]
    #[default]
    Note,

    /// Highlighted text selection
    #[db_rename = "highlight"]
    #[serde(rename = "highlight")]
    Highlight,

    /// Comment on specific content
    #[db_rename = "comment"]
    #[serde(rename = "comment")]
    Comment,
}

impl AnnotationType {
    /// Returns whether this is a note annotation.
    #[inline]
    pub fn is_note(self) -> bool {
        matches!(self, AnnotationType::Note)
    }

    /// Returns whether this is a highlight annotation.
    #[inline]
    pub fn is_highlight(self) -> bool {
        matches!(self, AnnotationType::Highlight)
    }

    /// Returns whether this is a comment annotation.
    #[inline]
    pub fn is_comment(self) -> bool {
        matches!(self, AnnotationType::Comment)
    }
}
