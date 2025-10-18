//! Document versions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::ConstraintCategory;

/// Document versions table constraint violations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString, Display, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentVersionConstraints {
    // Version validation constraints
    #[strum(serialize = "document_versions_version_number_min")]
    VersionNumberMin,
    #[strum(serialize = "document_versions_display_name_length_min")]
    DisplayNameLengthMin,
    #[strum(serialize = "document_versions_display_name_length_max")]
    DisplayNameLengthMax,
    #[strum(serialize = "document_versions_file_extension_format")]
    FileExtensionFormat,
    #[strum(serialize = "document_versions_mime_type_not_empty")]
    MimeTypeNotEmpty,

    // Version processing constraints
    #[strum(serialize = "document_versions_processing_credits_min")]
    ProcessingCreditsMin,
    #[strum(serialize = "document_versions_processing_duration_min")]
    ProcessingDurationMin,
    #[strum(serialize = "document_versions_processing_cost_min")]
    ProcessingCostMin,
    #[strum(serialize = "document_versions_api_calls_min")]
    ApiCallsMin,

    // Version quality score constraints
    #[strum(serialize = "document_versions_accuracy_score_min")]
    AccuracyScoreMin,
    #[strum(serialize = "document_versions_accuracy_score_max")]
    AccuracyScoreMax,
    #[strum(serialize = "document_versions_completeness_score_min")]
    CompletenessScoreMin,
    #[strum(serialize = "document_versions_completeness_score_max")]
    CompletenessScoreMax,
    #[strum(serialize = "document_versions_confidence_score_min")]
    ConfidenceScoreMin,
    #[strum(serialize = "document_versions_confidence_score_max")]
    ConfidenceScoreMax,

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
    #[strum(serialize = "document_versions_processing_results_size_min")]
    ProcessingResultsSizeMin,
    #[strum(serialize = "document_versions_processing_results_size_max")]
    ProcessingResultsSizeMax,
    #[strum(serialize = "document_versions_metadata_size_min")]
    MetadataSizeMin,
    #[strum(serialize = "document_versions_metadata_size_max")]
    MetadataSizeMax,

    // Version retention constraints
    #[strum(serialize = "document_versions_retention_period_min")]
    RetentionPeriodMin,
    #[strum(serialize = "document_versions_retention_period_max")]
    RetentionPeriodMax,

    // Version chronological constraints
    #[strum(serialize = "document_versions_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "document_versions_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "document_versions_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "document_versions_auto_delete_after_created")]
    AutoDeleteAfterCreated,

    // Version unique constraints
    #[strum(serialize = "document_versions_unique_version")]
    UniqueVersion,
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
            | DocumentVersionConstraints::DisplayNameLengthMin
            | DocumentVersionConstraints::DisplayNameLengthMax
            | DocumentVersionConstraints::FileExtensionFormat
            | DocumentVersionConstraints::MimeTypeNotEmpty
            | DocumentVersionConstraints::ProcessingCreditsMin
            | DocumentVersionConstraints::ProcessingDurationMin
            | DocumentVersionConstraints::ProcessingCostMin
            | DocumentVersionConstraints::ApiCallsMin
            | DocumentVersionConstraints::AccuracyScoreMin
            | DocumentVersionConstraints::AccuracyScoreMax
            | DocumentVersionConstraints::CompletenessScoreMin
            | DocumentVersionConstraints::CompletenessScoreMax
            | DocumentVersionConstraints::ConfidenceScoreMin
            | DocumentVersionConstraints::ConfidenceScoreMax
            | DocumentVersionConstraints::FileSizeMin
            | DocumentVersionConstraints::StoragePathNotEmpty
            | DocumentVersionConstraints::StorageBucketNotEmpty
            | DocumentVersionConstraints::FileHashSha256Length
            | DocumentVersionConstraints::ProcessingResultsSizeMin
            | DocumentVersionConstraints::ProcessingResultsSizeMax
            | DocumentVersionConstraints::MetadataSizeMin
            | DocumentVersionConstraints::MetadataSizeMax
            | DocumentVersionConstraints::RetentionPeriodMin
            | DocumentVersionConstraints::RetentionPeriodMax => ConstraintCategory::Validation,

            DocumentVersionConstraints::UpdatedAfterCreated
            | DocumentVersionConstraints::DeletedAfterCreated
            | DocumentVersionConstraints::DeletedAfterUpdated
            | DocumentVersionConstraints::AutoDeleteAfterCreated => {
                ConstraintCategory::Chronological
            }

            DocumentVersionConstraints::UniqueVersion => ConstraintCategory::Uniqueness,
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
