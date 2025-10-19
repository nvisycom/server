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
    #[strum(serialize = "projects_display_name_length_min")]
    DisplayNameLengthMin,
    #[strum(serialize = "projects_display_name_length_max")]
    DisplayNameLengthMax,
    #[strum(serialize = "projects_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "projects_project_code_format")]
    ProjectCodeFormat,
    #[strum(serialize = "projects_category_length_max")]
    CategoryLengthMax,
    #[strum(serialize = "projects_tags_count_max")]
    TagsCountMax,

    // Project resource constraints
    #[strum(serialize = "projects_keep_for_sec_min")]
    KeepForSecMin,
    #[strum(serialize = "projects_keep_for_sec_max")]
    KeepForSecMax,
    #[strum(serialize = "projects_max_members_min")]
    MaxMembersMin,
    #[strum(serialize = "projects_max_members_max")]
    MaxMembersMax,
    #[strum(serialize = "projects_max_documents_min")]
    MaxDocumentsMin,
    #[strum(serialize = "projects_max_storage_mb_min")]
    MaxStorageMbMin,

    // Project metadata constraints
    #[strum(serialize = "projects_metadata_size_min")]
    MetadataSizeMin,
    #[strum(serialize = "projects_metadata_size_max")]
    MetadataSizeMax,
    #[strum(serialize = "projects_settings_size_min")]
    SettingsSizeMin,
    #[strum(serialize = "projects_settings_size_max")]
    SettingsSizeMax,

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
    #[strum(serialize = "projects_template_cannot_have_template")]
    TemplateCannotHaveTemplate,
    #[strum(serialize = "projects_active_status_not_archived")]
    ActiveStatusNotArchived,
    #[strum(serialize = "projects_archive_status_consistency")]
    ArchiveStatusConsistency,

    // Project unique constraints
    #[strum(serialize = "projects_display_name_owner_unique_idx")]
    DisplayNameOwnerUnique,
    #[strum(serialize = "projects_project_code_unique_idx")]
    ProjectCodeUnique,
}

impl ProjectConstraints {
    /// Creates a new [`ProjectConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectConstraints::DisplayNameLengthMin
            | ProjectConstraints::DisplayNameLengthMax
            | ProjectConstraints::DescriptionLengthMax
            | ProjectConstraints::ProjectCodeFormat
            | ProjectConstraints::CategoryLengthMax
            | ProjectConstraints::TagsCountMax
            | ProjectConstraints::KeepForSecMin
            | ProjectConstraints::KeepForSecMax
            | ProjectConstraints::MaxMembersMin
            | ProjectConstraints::MaxMembersMax
            | ProjectConstraints::MaxDocumentsMin
            | ProjectConstraints::MaxStorageMbMin
            | ProjectConstraints::MetadataSizeMin
            | ProjectConstraints::MetadataSizeMax
            | ProjectConstraints::SettingsSizeMin
            | ProjectConstraints::SettingsSizeMax => ConstraintCategory::Validation,

            ProjectConstraints::UpdatedAfterCreated
            | ProjectConstraints::DeletedAfterCreated
            | ProjectConstraints::DeletedAfterUpdated
            | ProjectConstraints::ArchivedAfterCreated
            | ProjectConstraints::DeletedAfterArchived => ConstraintCategory::Chronological,

            ProjectConstraints::TemplateCannotHaveTemplate
            | ProjectConstraints::ActiveStatusNotArchived
            | ProjectConstraints::ArchiveStatusConsistency => ConstraintCategory::BusinessLogic,

            ProjectConstraints::DisplayNameOwnerUnique | ProjectConstraints::ProjectCodeUnique => {
                ConstraintCategory::Uniqueness
            }
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
