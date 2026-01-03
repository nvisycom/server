//! Workspaces table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceConstraints {
    // Workspace validation constraints
    #[strum(serialize = "workspaces_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspaces_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "workspaces_keep_for_sec_range")]
    KeepForSecRange,
    #[strum(serialize = "workspaces_max_members_min")]
    MaxMembersMin,
    #[strum(serialize = "workspaces_max_members_max")]
    MaxMembersMax,
    #[strum(serialize = "workspaces_max_storage_min")]
    MaxStorageMin,
    #[strum(serialize = "workspaces_tags_count_max")]
    TagsCountMax,
    #[strum(serialize = "workspaces_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspaces_settings_size")]
    SettingsSize,

    // Workspace chronological constraints
    #[strum(serialize = "workspaces_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspaces_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "workspaces_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "workspaces_archived_after_created")]
    ArchivedAfterCreated,
    #[strum(serialize = "workspaces_deleted_after_archived")]
    DeletedAfterArchived,

    // Workspace business logic constraints
    #[strum(serialize = "workspaces_active_status_not_archived")]
    ActiveStatusNotArchived,
    #[strum(serialize = "workspaces_archive_status_consistency")]
    ArchiveStatusConsistency,
}

impl WorkspaceConstraints {
    /// Creates a new [`WorkspaceConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceConstraints::DisplayNameLength
            | WorkspaceConstraints::DescriptionLengthMax
            | WorkspaceConstraints::KeepForSecRange
            | WorkspaceConstraints::MaxMembersMin
            | WorkspaceConstraints::MaxMembersMax
            | WorkspaceConstraints::MaxStorageMin
            | WorkspaceConstraints::TagsCountMax
            | WorkspaceConstraints::MetadataSize
            | WorkspaceConstraints::SettingsSize => ConstraintCategory::Validation,

            WorkspaceConstraints::UpdatedAfterCreated
            | WorkspaceConstraints::DeletedAfterCreated
            | WorkspaceConstraints::DeletedAfterUpdated
            | WorkspaceConstraints::ArchivedAfterCreated
            | WorkspaceConstraints::DeletedAfterArchived => ConstraintCategory::Chronological,

            WorkspaceConstraints::ActiveStatusNotArchived
            | WorkspaceConstraints::ArchiveStatusConsistency => ConstraintCategory::BusinessLogic,
        }
    }
}

impl From<WorkspaceConstraints> for String {
    #[inline]
    fn from(val: WorkspaceConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
