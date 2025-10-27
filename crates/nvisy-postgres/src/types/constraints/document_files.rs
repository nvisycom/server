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
    #[strum(serialize = "document_files_display_name_length_min")]
    DisplayNameLengthMin,
    #[strum(serialize = "document_files_display_name_length_max")]
    DisplayNameLengthMax,
    #[strum(serialize = "document_files_original_filename_length_min")]
    OriginalFilenameLengthMin,
    #[strum(serialize = "document_files_original_filename_length_max")]
    OriginalFilenameLengthMax,
    #[strum(serialize = "document_files_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "document_files_mime_type_length_min")]
    MimeTypeLengthMin,
    #[strum(serialize = "document_files_mime_type_length_max")]
    MimeTypeLengthMax,

    // File processing constraints
    #[strum(serialize = "document_files_processing_priority_min")]
    ProcessingPriorityMin,
    #[strum(serialize = "document_files_processing_priority_max")]
    ProcessingPriorityMax,
    #[strum(serialize = "document_files_processing_attempts_min")]
    ProcessingAttemptsMin,
    #[strum(serialize = "document_files_processing_attempts_max")]
    ProcessingAttemptsMax,
    #[strum(serialize = "document_files_processing_error_length_max")]
    ProcessingErrorLengthMax,
    #[strum(serialize = "document_files_processing_duration_min")]
    ProcessingDurationMin,

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
    #[strum(serialize = "document_files_metadata_size_min")]
    MetadataSizeMin,
    #[strum(serialize = "document_files_metadata_size_max")]
    MetadataSizeMax,

    // File quality score constraints
    #[strum(serialize = "document_files_processing_score_min")]
    ProcessingScoreMin,
    #[strum(serialize = "document_files_processing_score_max")]
    ProcessingScoreMax,
    #[strum(serialize = "document_files_completeness_score_min")]
    CompletenessScoreMin,
    #[strum(serialize = "document_files_completeness_score_max")]
    CompletenessScoreMax,
    #[strum(serialize = "document_files_confidence_score_min")]
    ConfidenceScoreMin,
    #[strum(serialize = "document_files_confidence_score_max")]
    ConfidenceScoreMax,

    // File retention constraints
    #[strum(serialize = "document_files_retention_period_min")]
    RetentionPeriodMin,
    #[strum(serialize = "document_files_retention_period_max")]
    RetentionPeriodMax,

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
            DocumentFileConstraints::DisplayNameLengthMin
            | DocumentFileConstraints::DisplayNameLengthMax
            | DocumentFileConstraints::OriginalFilenameLengthMin
            | DocumentFileConstraints::OriginalFilenameLengthMax
            | DocumentFileConstraints::FileExtensionFormat
            | DocumentFileConstraints::MimeTypeLengthMin
            | DocumentFileConstraints::MimeTypeLengthMax
            | DocumentFileConstraints::ProcessingPriorityMin
            | DocumentFileConstraints::ProcessingPriorityMax
            | DocumentFileConstraints::ProcessingAttemptsMin
            | DocumentFileConstraints::ProcessingAttemptsMax
            | DocumentFileConstraints::ProcessingErrorLengthMax
            | DocumentFileConstraints::ProcessingDurationMin
            | DocumentFileConstraints::FileSizeMin
            | DocumentFileConstraints::StoragePathNotEmpty
            | DocumentFileConstraints::StorageBucketNotEmpty
            | DocumentFileConstraints::FileHashSha256Length
            | DocumentFileConstraints::MetadataSizeMin
            | DocumentFileConstraints::MetadataSizeMax
            | DocumentFileConstraints::ProcessingScoreMin
            | DocumentFileConstraints::ProcessingScoreMax
            | DocumentFileConstraints::CompletenessScoreMin
            | DocumentFileConstraints::CompletenessScoreMax
            | DocumentFileConstraints::ConfidenceScoreMin
            | DocumentFileConstraints::ConfidenceScoreMax
            | DocumentFileConstraints::RetentionPeriodMin
            | DocumentFileConstraints::RetentionPeriodMax => ConstraintCategory::Validation,

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
