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
            WorkspaceFileConstraints::FileSizeMin => {
                ErrorKind::BadRequest.with_message("File size must be greater than or equal to 0")
            }
            WorkspaceFileConstraints::MetadataSize => {
                ErrorKind::BadRequest.with_message("File metadata size is invalid")
            }
            WorkspaceFileConstraints::VersionNumberMin => {
                ErrorKind::BadRequest.with_message("Version number must be at least 1")
            }
            WorkspaceFileConstraints::SourceObjectUnique => {
                ErrorKind::Conflict.with_message("This source object has already been imported")
            }
            WorkspaceFileConstraints::WorkspaceIdIdUnique => {
                ErrorKind::Conflict.with_message("A file with this identifier already exists")
            }
        };

        error.with_resource("file")
    }
}
