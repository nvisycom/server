//! Files table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Files table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceFileConstraints {
    // File identity validation constraints
    #[strum(serialize = "workspace_files_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspace_files_original_filename_length")]
    OriginalFilenameLength,
    #[strum(serialize = "workspace_files_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "workspace_files_mime_type_format")]
    MimeTypeFormat,
    #[strum(serialize = "workspace_files_tags_count_max")]
    TagsCountMax,

    // File storage constraints
    #[strum(serialize = "workspace_files_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "workspace_files_storage_path_not_empty")]
    StoragePathNotEmpty,
    #[strum(serialize = "workspace_files_storage_bucket_not_empty")]
    StorageBucketNotEmpty,
    #[strum(serialize = "workspace_files_file_hash_sha256_length")]
    FileHashSha256Length,

    // File metadata constraints
    #[strum(serialize = "workspace_files_metadata_size")]
    MetadataSize,

    // File version constraints
    #[strum(serialize = "workspace_files_version_number_min")]
    VersionNumberMin,

    // Uniqueness constraints
    #[strum(serialize = "workspace_files_workspace_id_id_key")]
    WorkspaceIdIdUnique,

    // File chronological constraints
    #[strum(serialize = "workspace_files_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_files_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "workspace_files_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl WorkspaceFileConstraints {
    /// Creates a new [`WorkspaceFileConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceFileConstraints::DisplayNameLength
            | WorkspaceFileConstraints::OriginalFilenameLength
            | WorkspaceFileConstraints::FileExtensionFormat
            | WorkspaceFileConstraints::MimeTypeFormat
            | WorkspaceFileConstraints::TagsCountMax
            | WorkspaceFileConstraints::FileSizeMin
            | WorkspaceFileConstraints::StoragePathNotEmpty
            | WorkspaceFileConstraints::StorageBucketNotEmpty
            | WorkspaceFileConstraints::FileHashSha256Length
            | WorkspaceFileConstraints::MetadataSize
            | WorkspaceFileConstraints::VersionNumberMin => ConstraintCategory::Validation,

            WorkspaceFileConstraints::WorkspaceIdIdUnique => ConstraintCategory::Uniqueness,

            WorkspaceFileConstraints::UpdatedAfterCreated
            | WorkspaceFileConstraints::DeletedAfterCreated
            | WorkspaceFileConstraints::DeletedAfterUpdated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspaceFileConstraints> for String {
    #[inline]
    fn from(val: WorkspaceFileConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceFileConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
