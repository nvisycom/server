//! Studio-related constraint violation error handlers.

use nvisy_postgres::types::{
    StudioOperationConstraints, StudioSessionConstraints, StudioToolCallConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<StudioSessionConstraints> for Error<'static> {
    fn from(c: StudioSessionConstraints) -> Self {
        let error = match c {
            StudioSessionConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Session name must be between 1 and 255 characters long"),
            StudioSessionConstraints::ModelConfigSize => {
                ErrorKind::BadRequest.with_message("Model configuration size is invalid")
            }
            StudioSessionConstraints::MessageCountMin => {
                ErrorKind::InternalServerError.into_error()
            }
            StudioSessionConstraints::TokenCountMin => ErrorKind::InternalServerError.into_error(),
            StudioSessionConstraints::UpdatedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("studio_session")
    }
}

impl From<StudioToolCallConstraints> for Error<'static> {
    fn from(c: StudioToolCallConstraints) -> Self {
        let error = match c {
            StudioToolCallConstraints::ToolNameLength => ErrorKind::BadRequest
                .with_message("Tool name must be between 1 and 128 characters long"),
            StudioToolCallConstraints::ToolInputSize => {
                ErrorKind::BadRequest.with_message("Tool input size exceeds maximum allowed")
            }
            StudioToolCallConstraints::ToolOutputSize => {
                ErrorKind::BadRequest.with_message("Tool output size exceeds maximum allowed")
            }
            StudioToolCallConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("studio_tool_call")
    }
}

impl From<StudioOperationConstraints> for Error<'static> {
    fn from(c: StudioOperationConstraints) -> Self {
        let error = match c {
            StudioOperationConstraints::OperationTypeLength => ErrorKind::BadRequest
                .with_message("Operation type must be between 1 and 64 characters long"),
            StudioOperationConstraints::OperationDiffSize => {
                ErrorKind::BadRequest.with_message("Operation diff size exceeds maximum allowed")
            }
            StudioOperationConstraints::RevertRequiresApplied => ErrorKind::BadRequest
                .with_message("Cannot revert an operation that has not been applied"),
            StudioOperationConstraints::AppliedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("studio_operation")
    }
}
