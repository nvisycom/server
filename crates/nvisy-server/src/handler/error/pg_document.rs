//! File-related constraint violation error handlers.

use nvisy_postgres::types::FileConstraints;

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
