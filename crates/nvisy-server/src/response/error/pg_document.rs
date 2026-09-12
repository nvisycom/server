//! Document- and blob-related constraint violation error handlers.

use nvisy_postgres::types::{WorkspaceBlobConstraints, WorkspaceDocumentConstraints};

use super::{Error, ErrorKind};

impl From<WorkspaceDocumentConstraints> for Error<'static> {
    fn from(c: WorkspaceDocumentConstraints) -> Self {
        let error = match c {
            WorkspaceDocumentConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Document name must be between 1 and 255 characters long"),
            WorkspaceDocumentConstraints::OriginalFilenameLength => ErrorKind::BadRequest
                .with_message("Original filename must be between 1 and 255 characters long"),
            WorkspaceDocumentConstraints::FileExtensionFormat => {
                ErrorKind::BadRequest.with_message("Invalid file extension format")
            }
            WorkspaceDocumentConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("Document metadata size is invalid")
            }
            WorkspaceDocumentConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A document with this identifier already exists")
            }
        };

        error.with_resource("document")
    }
}

impl From<WorkspaceBlobConstraints> for Error<'static> {
    fn from(c: WorkspaceBlobConstraints) -> Self {
        let error = match c {
            WorkspaceBlobConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than or equal to 0")
            }
            WorkspaceBlobConstraints::ContentHashLength
            | WorkspaceBlobConstraints::StoragePathNotEmpty
            | WorkspaceBlobConstraints::StorageBucketNotEmpty
            | WorkspaceBlobConstraints::RefCountMin
            | WorkspaceBlobConstraints::ExpiresAfterCreated
            | WorkspaceBlobConstraints::PurgedAfterCreated => {
                ErrorKind::InternalServerError.with_message("Invalid blob state")
            }
        };

        error.with_resource("blob")
    }
}
