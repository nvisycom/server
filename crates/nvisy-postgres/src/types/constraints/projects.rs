//! Projects table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectConstraints {
    // Project validation constraints
    #[strum(serialize = "projects_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "projects_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "projects_keep_for_sec_range")]
    KeepForSecRange,
    #[strum(serialize = "projects_max_members_min")]
    MaxMembersMin,
    #[strum(serialize = "projects_max_members_max")]
    MaxMembersMax,
    #[strum(serialize = "projects_max_storage_min")]
    MaxStorageMin,
    #[strum(serialize = "projects_tags_count_max")]
    TagsCountMax,
    #[strum(serialize = "projects_metadata_size")]
    MetadataSize,
    #[strum(serialize = "projects_settings_size")]
    SettingsSize,

    // Project chronological constraints
    #[strum(serialize = "projects_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "projects_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "projects_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "projects_archived_after_created")]
    ArchivedAfterCreated,
    #[strum(serialize = "projects_deleted_after_archived")]
    DeletedAfterArchived,

    // Project business logic constraints
    #[strum(serialize = "projects_active_status_not_archived")]
    ActiveStatusNotArchived,
    #[strum(serialize = "projects_archive_status_consistency")]
    ArchiveStatusConsistency,
}

impl ProjectConstraints {
    /// Creates a new [`ProjectConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectConstraints::DisplayNameLength
            | ProjectConstraints::DescriptionLengthMax
            | ProjectConstraints::KeepForSecRange
            | ProjectConstraints::MaxMembersMin
            | ProjectConstraints::MaxMembersMax
            | ProjectConstraints::MaxStorageMin
            | ProjectConstraints::TagsCountMax
            | ProjectConstraints::MetadataSize
            | ProjectConstraints::SettingsSize => ConstraintCategory::Validation,

            ProjectConstraints::UpdatedAfterCreated
            | ProjectConstraints::DeletedAfterCreated
            | ProjectConstraints::DeletedAfterUpdated
            | ProjectConstraints::ArchivedAfterCreated
            | ProjectConstraints::DeletedAfterArchived => ConstraintCategory::Chronological,

            ProjectConstraints::ActiveStatusNotArchived
            | ProjectConstraints::ArchiveStatusConsistency => ConstraintCategory::BusinessLogic,
        }
    }
}

impl From<ProjectConstraints> for String {
    #[inline]
    fn from(val: ProjectConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
