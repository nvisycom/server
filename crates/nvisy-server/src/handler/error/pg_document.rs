//! File-related constraint violation error handlers.

use nvisy_postgres::types::{FileAnnotationConstraints, FileChunkConstraints, FileConstraints};

use crate::handler::{Error, ErrorKind};

impl From<FileConstraints> for Error<'static> {
    fn from(c: FileConstraints) -> Self {
        let error = match c {
            FileConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("File name must be between 1 and 255 characters long"),
            FileConstraints::OriginalFilenameLength => ErrorKind::BadRequest
                .with_message("Original filename must be between 1 and 255 characters long"),
            FileConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            FileConstraints::MimeTypeFormat => {
                ErrorKind::BadRequest.with_message("Invalid MIME type format")
            }
            FileConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than or equal to 0")
            }
            FileConstraints::StoragePathNotEmpty => ErrorKind::InternalServerError.into_error(),
            FileConstraints::StorageBucketNotEmpty => ErrorKind::InternalServerError.into_error(),
            FileConstraints::FileHashSha256Length => ErrorKind::InternalServerError.into_error(),
            FileConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("File metadata size is invalid")
            }
            FileConstraints::TagsCountMax => {
                ErrorKind::BadRequest.with_message("Maximum number of tags exceeded")
            }
            FileConstraints::VersionNumberMin => {
                ErrorKind::BadRequest.with_message("Version number must be at least 1")
            }
            FileConstraints::UpdatedAfterCreated
            | FileConstraints::DeletedAfterCreated
            | FileConstraints::DeletedAfterUpdated => ErrorKind::InternalServerError.into_error(),
        };

        error.with_resource("file")
    }
}

impl From<FileAnnotationConstraints> for Error<'static> {
    fn from(c: FileAnnotationConstraints) -> Self {
        let error = match c {
            FileAnnotationConstraints::ContentLength => {
                ErrorKind::BadRequest.with_message("Annotation content length is invalid")
            }
            FileAnnotationConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Annotation metadata size is invalid")
            }
            FileAnnotationConstraints::UpdatedAfterCreated
            | FileAnnotationConstraints::DeletedAfterCreated
            | FileAnnotationConstraints::DeletedAfterUpdated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("file_annotation")
    }
}

impl From<FileChunkConstraints> for Error<'static> {
    fn from(c: FileChunkConstraints) -> Self {
        let error = match c {
            FileChunkConstraints::ChunkIndexMin => {
                ErrorKind::BadRequest.with_message("Chunk index must be at least 0")
            }
            FileChunkConstraints::ContentSha256Length => {
                ErrorKind::InternalServerError.into_error()
            }
            FileChunkConstraints::ContentSizeMin => {
                ErrorKind::BadRequest.with_message("Chunk content size must be at least 0")
            }
            FileChunkConstraints::TokenCountMin => {
                ErrorKind::BadRequest.with_message("Token count must be at least 0")
            }
            FileChunkConstraints::EmbeddingModelFormat => {
                ErrorKind::BadRequest.with_message("Invalid embedding model format")
            }
            FileChunkConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Chunk metadata size is invalid")
            }
            FileChunkConstraints::UpdatedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            FileChunkConstraints::FileChunkUnique => {
                ErrorKind::Conflict.with_message("Chunk with this index already exists for file")
            }
        };

        error.with_resource("file_chunk")
    }
}
