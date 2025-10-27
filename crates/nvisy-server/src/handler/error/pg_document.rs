//! Document-related constraint violation error handlers.

use nvisy_postgres::types::{
    DocumentConstraints, DocumentFileConstraints, DocumentVersionConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<DocumentConstraints> for Error<'static> {
    fn from(c: DocumentConstraints) -> Self {
        let error =
            match c {
                DocumentConstraints::DisplayNameLengthMin => ErrorKind::BadRequest
                    .with_message("Document name must be at least 3 characters long"),
                DocumentConstraints::DisplayNameLengthMax => {
                    ErrorKind::BadRequest.with_message("Document name cannot exceed 240 characters")
                }
                DocumentConstraints::DescriptionLengthMax => {
                    ErrorKind::BadRequest.with_message("Document description is too long")
                }
                DocumentConstraints::TagsCountMax => {
                    ErrorKind::BadRequest.with_message("Too many tags")
                }
                DocumentConstraints::MetadataSizeMin => ErrorKind::InternalServerError.into_error(),
                DocumentConstraints::MetadataSizeMax => ErrorKind::BadRequest
                    .with_message("Document metadata exceeds maximum allowed size"),
                DocumentConstraints::SettingsSizeMin => ErrorKind::InternalServerError.into_error(),
                DocumentConstraints::SettingsSizeMax => ErrorKind::BadRequest
                    .with_message("Document settings exceed maximum allowed size"),
                DocumentConstraints::UpdatedAfterCreated
                | DocumentConstraints::DeletedAfterCreated
                | DocumentConstraints::DeletedAfterUpdated => {
                    ErrorKind::InternalServerError.into_error()
                }
            };

        error.with_resource("document")
    }
}

impl From<DocumentFileConstraints> for Error<'static> {
    fn from(c: DocumentFileConstraints) -> Self {
        let error = match c {
            DocumentFileConstraints::DisplayNameLengthMin => {
                ErrorKind::BadRequest.with_message("File name must be at least 2 characters long")
            }
            DocumentFileConstraints::DisplayNameLengthMax => {
                ErrorKind::BadRequest.with_message("File name cannot exceed 240 characters")
            }
            DocumentFileConstraints::OriginalFilenameLengthMin => {
                ErrorKind::BadRequest.with_message("Original filename is too short")
            }
            DocumentFileConstraints::OriginalFilenameLengthMax => {
                ErrorKind::BadRequest.with_message("Original filename is too long")
            }
            DocumentFileConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            DocumentFileConstraints::MimeTypeLengthMin => {
                ErrorKind::BadRequest.with_message("MIME type is too short")
            }
            DocumentFileConstraints::MimeTypeLengthMax => {
                ErrorKind::BadRequest.with_message("MIME type is too long")
            }
            DocumentFileConstraints::ProcessingPriorityMin => {
                ErrorKind::BadRequest.with_message("Processing priority is too low")
            }
            DocumentFileConstraints::ProcessingPriorityMax => {
                ErrorKind::BadRequest.with_message("Processing priority is too high")
            }
            DocumentFileConstraints::ProcessingAttemptsMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::ProcessingAttemptsMax => {
                ErrorKind::BadRequest.with_message("Too many processing attempts")
            }
            DocumentFileConstraints::ProcessingErrorLengthMax => {
                ErrorKind::BadRequest.with_message("Processing error message is too long")
            }
            DocumentFileConstraints::ProcessingDurationMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than 0")
            }
            DocumentFileConstraints::StoragePathNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::StorageBucketNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::FileHashSha256Length => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::MetadataSizeMin => ErrorKind::InternalServerError.into_error(),
            DocumentFileConstraints::MetadataSizeMax => {
                ErrorKind::BadRequest.with_message("File metadata is too large")
            }
            DocumentFileConstraints::ProcessingScoreMin
            | DocumentFileConstraints::ProcessingScoreMax
            | DocumentFileConstraints::CompletenessScoreMin
            | DocumentFileConstraints::CompletenessScoreMax
            | DocumentFileConstraints::ConfidenceScoreMin
            | DocumentFileConstraints::ConfidenceScoreMax => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentFileConstraints::RetentionPeriodMin => ErrorKind::BadRequest
                .with_message("File retention period must be at least 1 second"),
            DocumentFileConstraints::RetentionPeriodMax => {
                ErrorKind::BadRequest.with_message("File retention period cannot exceed 7 days")
            }
            DocumentFileConstraints::UpdatedAfterCreated
            | DocumentFileConstraints::DeletedAfterCreated
            | DocumentFileConstraints::DeletedAfterUpdated
            | DocumentFileConstraints::AutoDeleteAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("document_file")
    }
}

impl From<DocumentVersionConstraints> for Error<'static> {
    fn from(c: DocumentVersionConstraints) -> Self {
        let error = match c {
            DocumentVersionConstraints::VersionNumberMin => {
                ErrorKind::BadRequest.with_message("Version number must be at least 1")
            }
            DocumentVersionConstraints::DisplayNameLengthMin => ErrorKind::BadRequest
                .with_message("Version name must be at least 2 characters long"),
            DocumentVersionConstraints::DisplayNameLengthMax => {
                ErrorKind::BadRequest.with_message("Version name cannot exceed 240 characters")
            }
            DocumentVersionConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            DocumentVersionConstraints::MimeTypeNotEmpty => {
                ErrorKind::BadRequest.with_message("MIME type cannot be empty")
            }
            DocumentVersionConstraints::ProcessingCreditsMin
            | DocumentVersionConstraints::ProcessingDurationMin
            | DocumentVersionConstraints::ProcessingCostMin
            | DocumentVersionConstraints::ApiCallsMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::AccuracyScoreMin
            | DocumentVersionConstraints::AccuracyScoreMax
            | DocumentVersionConstraints::CompletenessScoreMin
            | DocumentVersionConstraints::CompletenessScoreMax
            | DocumentVersionConstraints::ConfidenceScoreMin
            | DocumentVersionConstraints::ConfidenceScoreMax => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than 0")
            }
            DocumentVersionConstraints::StoragePathNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::StorageBucketNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::FileHashSha256Length => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::ProcessingResultsSizeMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::ProcessingResultsSizeMax => {
                ErrorKind::BadRequest.with_message("Processing results are too large")
            }
            DocumentVersionConstraints::MetadataSizeMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::MetadataSizeMax => {
                ErrorKind::BadRequest.with_message("Version metadata is too large")
            }
            DocumentVersionConstraints::RetentionPeriodMin => ErrorKind::BadRequest
                .with_message("Version retention period must be at least 1 second"),
            DocumentVersionConstraints::RetentionPeriodMax => {
                ErrorKind::BadRequest.with_message("Version retention period cannot exceed 7 days")
            }
            DocumentVersionConstraints::UpdatedAfterCreated
            | DocumentVersionConstraints::DeletedAfterCreated
            | DocumentVersionConstraints::DeletedAfterUpdated
            | DocumentVersionConstraints::AutoDeleteAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::UniqueVersion => {
                ErrorKind::Conflict.with_message("A version with this number already exists")
            }
        };

        error.with_resource("document_version")
    }
}
