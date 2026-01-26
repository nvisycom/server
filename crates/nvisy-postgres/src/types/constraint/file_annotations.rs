//! File annotations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// File annotations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum FileAnnotationConstraints {
    // Annotation content constraints
    #[strum(serialize = "file_annotations_content_length")]
    ContentLength,

    // Annotation metadata constraints
    #[strum(serialize = "file_annotations_metadata_size")]
    MetadataSize,

    // Annotation chronological constraints
    #[strum(serialize = "file_annotations_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "file_annotations_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "file_annotations_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl FileAnnotationConstraints {
    /// Creates a new [`FileAnnotationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            FileAnnotationConstraints::ContentLength | FileAnnotationConstraints::MetadataSize => {
                ConstraintCategory::Validation
            }

            FileAnnotationConstraints::UpdatedAfterCreated
            | FileAnnotationConstraints::DeletedAfterCreated
            | FileAnnotationConstraints::DeletedAfterUpdated => ConstraintCategory::Chronological,
        }
    }
}

impl From<FileAnnotationConstraints> for String {
    #[inline]
    fn from(val: FileAnnotationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for FileAnnotationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
