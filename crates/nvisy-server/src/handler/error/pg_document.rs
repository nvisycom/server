//! File-related constraint violation error handlers.

use nvisy_postgres::types::WorkspaceFileConstraints;

use crate::handler::{Error, ErrorKind};

impl From<WorkspaceFileConstraints> for Error<'static> {
    fn from(c: WorkspaceFileConstraints) -> Self {
        let error = match c {
            WorkspaceFileConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("File name must be between 1 and 255 characters long"),
            WorkspaceFileConstraints::OriginalFilenameLength => ErrorKind::BadRequest
                .with_message("Original filename must be between 1 and 255 characters long"),
            WorkspaceFileConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            WorkspaceFileConstraints::MimeTypeFormat => {
                ErrorKind::BadRequest.with_message("Invalid MIME type format")
            }
            WorkspaceFileConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than or equal to 0")
            }
            WorkspaceFileConstraints::StoragePathNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceFileConstraints::StorageBucketNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceFileConstraints::FileHashSha256Length => {
                ErrorKind::InternalServerError.into_error()
            }
            WorkspaceFileConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("File metadata size is invalid")
            }
            WorkspaceFileConstraints::TagsCountMax => {
                ErrorKind::BadRequest.with_message("Maximum number of tags exceeded")
            }
            WorkspaceFileConstraints::VersionNumberMin => {
                ErrorKind::BadRequest.with_message("Version number must be at least 1")
            }
            WorkspaceFileConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A file with this identifier already exists")
            }
            WorkspaceFileConstraints::UpdatedAfterCreated
            | WorkspaceFileConstraints::DeletedAfterCreated
            | WorkspaceFileConstraints::DeletedAfterUpdated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("file")
    }
}
