//! Files table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Files table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum FileConstraints {
    // File identity validation constraints
    #[strum(serialize = "files_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "files_original_filename_length")]
    OriginalFilenameLength,
    #[strum(serialize = "files_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "files_mime_type_format")]
    MimeTypeFormat,
    #[strum(serialize = "files_tags_count_max")]
    TagsCountMax,

    // File storage constraints
    #[strum(serialize = "files_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "files_storage_path_not_empty")]
    StoragePathNotEmpty,
    #[strum(serialize = "files_storage_bucket_not_empty")]
    StorageBucketNotEmpty,
    #[strum(serialize = "files_file_hash_sha256_length")]
    FileHashSha256Length,

    // File metadata constraints
    #[strum(serialize = "files_metadata_size")]
    MetadataSize,

    // File version constraints
    #[strum(serialize = "files_version_number_min")]
    VersionNumberMin,

    // File chronological constraints
    #[strum(serialize = "files_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "files_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "files_deleted_after_updated")]
    DeletedAfterUpdated,
}

impl FileConstraints {
    /// Creates a new [`FileConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            FileConstraints::DisplayNameLength
            | FileConstraints::OriginalFilenameLength
            | FileConstraints::FileExtensionFormat
            | FileConstraints::MimeTypeFormat
            | FileConstraints::TagsCountMax
            | FileConstraints::FileSizeMin
            | FileConstraints::StoragePathNotEmpty
            | FileConstraints::StorageBucketNotEmpty
            | FileConstraints::FileHashSha256Length
            | FileConstraints::MetadataSize
            | FileConstraints::VersionNumberMin => ConstraintCategory::Validation,

            FileConstraints::UpdatedAfterCreated
            | FileConstraints::DeletedAfterCreated
            | FileConstraints::DeletedAfterUpdated => ConstraintCategory::Chronological,
        }
    }
}

impl From<FileConstraints> for String {
    #[inline]
    fn from(val: FileConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for FileConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
