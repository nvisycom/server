//! Document files table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document files table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentFileConstraints {
    // File identity validation constraints
    #[strum(serialize = "document_files_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "document_files_original_filename_length")]
    OriginalFilenameLength,
    #[strum(serialize = "document_files_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "document_files_tags_count_max")]
    TagsCountMax,

    // File processing constraints
    #[strum(serialize = "document_files_processing_priority_range")]
    ProcessingPriorityRange,

    // File storage constraints
    #[strum(serialize = "document_files_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "document_files_storage_path_not_empty")]
    StoragePathNotEmpty,
    #[strum(serialize = "document_files_storage_bucket_not_empty")]
    StorageBucketNotEmpty,
    #[strum(serialize = "document_files_file_hash_sha256_length")]
    FileHashSha256Length,

    // File metadata constraints
    #[strum(serialize = "document_files_metadata_size")]
    MetadataSize,

    // File retention constraints
    #[strum(serialize = "document_files_retention_period")]
    RetentionPeriod,

    // File version constraints
    #[strum(serialize = "document_files_version_number_min")]
    VersionNumberMin,
    #[strum(serialize = "document_files_parent_same_document")]
    ParentSameDocument,

    // File chronological constraints
    #[strum(serialize = "document_files_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "document_files_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "document_files_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "document_files_auto_delete_after_created")]
    AutoDeleteAfterCreated,
}

impl DocumentFileConstraints {
    /// Creates a new [`DocumentFileConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentFileConstraints::DisplayNameLength
            | DocumentFileConstraints::OriginalFilenameLength
            | DocumentFileConstraints::FileExtensionFormat
            | DocumentFileConstraints::TagsCountMax
            | DocumentFileConstraints::ProcessingPriorityRange
            | DocumentFileConstraints::FileSizeMin
            | DocumentFileConstraints::StoragePathNotEmpty
            | DocumentFileConstraints::StorageBucketNotEmpty
            | DocumentFileConstraints::FileHashSha256Length
            | DocumentFileConstraints::MetadataSize
            | DocumentFileConstraints::RetentionPeriod
            | DocumentFileConstraints::VersionNumberMin
            | DocumentFileConstraints::ParentSameDocument => ConstraintCategory::Validation,

            DocumentFileConstraints::UpdatedAfterCreated
            | DocumentFileConstraints::DeletedAfterCreated
            | DocumentFileConstraints::DeletedAfterUpdated
            | DocumentFileConstraints::AutoDeleteAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<DocumentFileConstraints> for String {
    #[inline]
    fn from(val: DocumentFileConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentFileConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
