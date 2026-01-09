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
    /// General text annotation
    #[db_rename = "annotation"]
    #[serde(rename = "annotation")]
    #[default]
    Annotation,

    /// Highlighted text selection
    #[db_rename = "highlight"]
    #[serde(rename = "highlight")]
    Highlight,
}

impl AnnotationType {
    /// Returns whether this is a text annotation.
    #[inline]
    pub fn is_annotation(self) -> bool {
        matches!(self, AnnotationType::Annotation)
    }

    /// Returns whether this is a highlight annotation.
    #[inline]
    pub fn is_highlight(self) -> bool {
        matches!(self, AnnotationType::Highlight)
    }
}
