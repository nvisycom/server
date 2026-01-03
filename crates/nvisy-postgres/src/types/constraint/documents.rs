//! Documents table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentConstraints {
    // Document validation constraints
    #[strum(serialize = "documents_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "documents_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "documents_tags_count_max")]
    TagsCountMax,

    // Document metadata constraints
    #[strum(serialize = "documents_metadata_size")]
    MetadataSize,
    #[strum(serialize = "documents_settings_size")]
    SettingsSize,

    // Document chronological constraints
    #[strum(serialize = "documents_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "documents_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "documents_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl DocumentConstraints {
    /// Creates a new [`DocumentConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentConstraints::DisplayNameLength
            | DocumentConstraints::DescriptionLengthMax
            | DocumentConstraints::TagsCountMax
            | DocumentConstraints::MetadataSize
            | DocumentConstraints::SettingsSize => ConstraintCategory::Validation,

            DocumentConstraints::UpdatedAfterCreated
            | DocumentConstraints::DeletedAfterCreated
            | DocumentConstraints::DeletedAfterUpdated => ConstraintCategory::Chronological,
        }
    }
}

impl From<DocumentConstraints> for String {
    #[inline]
    fn from(val: DocumentConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
