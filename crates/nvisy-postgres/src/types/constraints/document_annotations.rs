//! Document annotations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document annotations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentAnnotationConstraints {
    // Annotation content constraints
    #[strum(serialize = "document_annotations_content_length")]
    ContentLength,
    #[strum(serialize = "document_annotations_type_format")]
    TypeFormat,

    // Annotation metadata constraints
    #[strum(serialize = "document_annotations_metadata_size")]
    MetadataSize,

    // Annotation chronological constraints
    #[strum(serialize = "document_annotations_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "document_annotations_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "document_annotations_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl DocumentAnnotationConstraints {
    /// Creates a new [`DocumentAnnotationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentAnnotationConstraints::ContentLength
            | DocumentAnnotationConstraints::TypeFormat
            | DocumentAnnotationConstraints::MetadataSize => ConstraintCategory::Validation,

            DocumentAnnotationConstraints::UpdatedAfterCreated
            | DocumentAnnotationConstraints::DeletedAfterCreated
            | DocumentAnnotationConstraints::DeletedAfterUpdated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<DocumentAnnotationConstraints> for String {
    #[inline]
    fn from(val: DocumentAnnotationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentAnnotationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
