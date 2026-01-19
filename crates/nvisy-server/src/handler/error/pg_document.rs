//! Document-related constraint violation error handlers.

use nvisy_postgres::types::{
    DocumentAnnotationConstraints, DocumentChunkConstraints, DocumentCommentConstraints,
    DocumentConstraints, DocumentFileConstraints, DocumentVersionConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<DocumentConstraints> for Error<'static> {
    fn from(c: DocumentConstraints) -> Self {
        let error = match c {
            DocumentConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Document name must be between 1 and 255 characters long"),
            DocumentConstraints::DescriptionLengthMax => ErrorKind::BadRequest
                .with_message("Document description cannot exceed 2048 characters"),
            DocumentConstraints::TagsCountMax => {
                ErrorKind::BadRequest.with_message("Cannot have more than 32 tags")
            }
            DocumentConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Document metadata size is invalid")
            }
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
        let error =
            match c {
                DocumentFileConstraints::DisplayNameLength => ErrorKind::BadRequest
                    .with_message("File name must be between 1 and 255 characters long"),
                DocumentFileConstraints::OriginalFilenameLength => ErrorKind::BadRequest
                    .with_message("Original filename must be between 1 and 255 characters long"),
                DocumentFileConstraints::FileExtensionFormat => {
                    ErrorKind::BadRequest.with_message("Invalid file extension format")
                }
                DocumentFileConstraints::ProcessingPriorityRange => ErrorKind::BadRequest
                    .with_message("Processing priority must be between 1 and 10"),
                DocumentFileConstraints::FileSizeMin => ErrorKind::BadRequest
                    .with_message("File size must be greater than or equal to 0"),
                DocumentFileConstraints::StoragePathNotEmpty => {
                    ErrorKind::InternalServerError.into_error()
                }
                DocumentFileConstraints::StorageBucketNotEmpty => {
                    ErrorKind::InternalServerError.into_error()
                }
                DocumentFileConstraints::FileHashSha256Length => {
                    ErrorKind::InternalServerError.into_error()
                }
                DocumentFileConstraints::MetadataSize => {
                    ErrorKind::BadRequest.with_message("File metadata size is invalid")
                }
                DocumentFileConstraints::RetentionPeriod => ErrorKind::BadRequest
                    .with_message("File retention period must be between 1 hour and 5 years"),
                DocumentFileConstraints::TagsCountMax => {
                    ErrorKind::BadRequest.with_message("Maximum number of tags exceeded")
                }
                DocumentFileConstraints::VersionNumberMin => {
                    ErrorKind::BadRequest.with_message("Version number must be at least 1")
                }
                DocumentFileConstraints::ParentSameDocument => ErrorKind::BadRequest
                    .with_message("Parent file must belong to the same document"),
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
            DocumentVersionConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Version name must be between 1 and 255 characters long"),
            DocumentVersionConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            DocumentVersionConstraints::ProcessingCreditsMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::ProcessingDurationMin => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentVersionConstraints::ApiCallsMin => ErrorKind::InternalServerError.into_error(),
            DocumentVersionConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than or equal to 0")
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
            DocumentVersionConstraints::ResultsSize => {
                ErrorKind::BadRequest.with_message("Processing results size is invalid")
            }
            DocumentVersionConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Version metadata size is invalid")
            }
            DocumentVersionConstraints::RetentionPeriod => ErrorKind::BadRequest
                .with_message("Version retention period must be between 1 hour and 5 years"),
            DocumentVersionConstraints::UpdatedAfterCreated
            | DocumentVersionConstraints::DeletedAfterCreated
            | DocumentVersionConstraints::DeletedAfterUpdated
            | DocumentVersionConstraints::AutoDeleteAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("document_version")
    }
}

impl From<DocumentCommentConstraints> for Error<'static> {
    fn from(c: DocumentCommentConstraints) -> Self {
        let error = match c {
            DocumentCommentConstraints::ContentLength => ErrorKind::BadRequest
                .with_message("Comment content must be between 1 and 10,000 characters"),
            DocumentCommentConstraints::OneTarget => ErrorKind::BadRequest.with_message(
                "Comment must be attached to exactly one target (document, file, or version)",
            ),
            DocumentCommentConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Comment metadata size is invalid")
            }
            DocumentCommentConstraints::UpdatedAfterCreated
            | DocumentCommentConstraints::DeletedAfterCreated
            | DocumentCommentConstraints::DeletedAfterUpdated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("document_comment")
    }
}

impl From<DocumentAnnotationConstraints> for Error<'static> {
    fn from(c: DocumentAnnotationConstraints) -> Self {
        let error = match c {
            DocumentAnnotationConstraints::ContentLength => {
                ErrorKind::BadRequest.with_message("Annotation content length is invalid")
            }
            DocumentAnnotationConstraints::TypeFormat => {
                ErrorKind::BadRequest.with_message("Annotation type format is invalid")
            }
            DocumentAnnotationConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Annotation metadata size is invalid")
            }
            DocumentAnnotationConstraints::UpdatedAfterCreated
            | DocumentAnnotationConstraints::DeletedAfterCreated
            | DocumentAnnotationConstraints::DeletedAfterUpdated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("document_annotation")
    }
}

impl From<DocumentChunkConstraints> for Error<'static> {
    fn from(c: DocumentChunkConstraints) -> Self {
        let error = match c {
            DocumentChunkConstraints::ChunkIndexMin => {
                ErrorKind::BadRequest.with_message("Chunk index must be at least 0")
            }
            DocumentChunkConstraints::ContentSha256Length => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentChunkConstraints::ContentSizeMin => {
                ErrorKind::BadRequest.with_message("Chunk content size must be at least 0")
            }
            DocumentChunkConstraints::TokenCountMin => {
                ErrorKind::BadRequest.with_message("Token count must be at least 0")
            }
            DocumentChunkConstraints::EmbeddingModelFormat => {
                ErrorKind::BadRequest.with_message("Invalid embedding model format")
            }
            DocumentChunkConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Chunk metadata size is invalid")
            }
            DocumentChunkConstraints::UpdatedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            DocumentChunkConstraints::FileChunkUnique => {
                ErrorKind::Conflict.with_message("Chunk with this index already exists for file")
            }
        };

        error.with_resource("document_chunk")
    }
}
