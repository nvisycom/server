//! Document versions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document versions table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentVersionConstraints {
    // Version validation constraints
    #[strum(serialize = "document_versions_version_number_min")]
    VersionNumberMin,
    #[strum(serialize = "document_versions_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "document_versions_file_extension_format")]
    FileExtensionFormat,

    // Version processing constraints
    #[strum(serialize = "document_versions_processing_credits_min")]
    ProcessingCreditsMin,
    #[strum(serialize = "document_versions_processing_duration_min")]
    ProcessingDurationMin,
    #[strum(serialize = "document_versions_api_calls_min")]
    ApiCallsMin,

    // Version storage constraints
    #[strum(serialize = "document_versions_file_size_min")]
    FileSizeMin,
    #[strum(serialize = "document_versions_storage_path_not_empty")]
    StoragePathNotEmpty,
    #[strum(serialize = "document_versions_storage_bucket_not_empty")]
    StorageBucketNotEmpty,
    #[strum(serialize = "document_versions_file_hash_sha256_length")]
    FileHashSha256Length,

    // Version metadata constraints
    #[strum(serialize = "document_versions_results_size")]
    ResultsSize,
    #[strum(serialize = "document_versions_metadata_size")]
    MetadataSize,

    // Version retention constraints
    #[strum(serialize = "document_versions_retention_period")]
    RetentionPeriod,

    // Version chronological constraints
    #[strum(serialize = "document_versions_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "document_versions_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "document_versions_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "document_versions_auto_delete_after_created")]
    AutoDeleteAfterCreated,
}

impl DocumentVersionConstraints {
    /// Creates a new [`DocumentVersionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentVersionConstraints::VersionNumberMin
            | DocumentVersionConstraints::DisplayNameLength
            | DocumentVersionConstraints::FileExtensionFormat
            | DocumentVersionConstraints::ProcessingCreditsMin
            | DocumentVersionConstraints::ProcessingDurationMin
            | DocumentVersionConstraints::ApiCallsMin
            | DocumentVersionConstraints::FileSizeMin
            | DocumentVersionConstraints::StoragePathNotEmpty
            | DocumentVersionConstraints::StorageBucketNotEmpty
            | DocumentVersionConstraints::FileHashSha256Length
            | DocumentVersionConstraints::ResultsSize
            | DocumentVersionConstraints::MetadataSize
            | DocumentVersionConstraints::RetentionPeriod => ConstraintCategory::Validation,

            DocumentVersionConstraints::UpdatedAfterCreated
            | DocumentVersionConstraints::DeletedAfterCreated
            | DocumentVersionConstraints::DeletedAfterUpdated
            | DocumentVersionConstraints::AutoDeleteAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<DocumentVersionConstraints> for String {
    #[inline]
    fn from(val: DocumentVersionConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentVersionConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
