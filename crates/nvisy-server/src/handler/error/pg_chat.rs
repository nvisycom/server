//! Chat-related constraint violation error handlers.

use nvisy_postgres::types::{
    ChatOperationConstraints, ChatSessionConstraints, ChatToolCallConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<ChatSessionConstraints> for Error<'static> {
    fn from(c: ChatSessionConstraints) -> Self {
        let error = match c {
            ChatSessionConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Session name must be between 1 and 255 characters long"),
            ChatSessionConstraints::ModelConfigSize => {
                ErrorKind::BadRequest.with_message("Model configuration size is invalid")
            }
            ChatSessionConstraints::MessageCountMin => ErrorKind::InternalServerError.into_error(),
            ChatSessionConstraints::TokenCountMin => ErrorKind::InternalServerError.into_error(),
            ChatSessionConstraints::UpdatedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("chat_session")
    }
}

impl From<ChatToolCallConstraints> for Error<'static> {
    fn from(c: ChatToolCallConstraints) -> Self {
        let error = match c {
            ChatToolCallConstraints::ToolNameLength => ErrorKind::BadRequest
                .with_message("Tool name must be between 1 and 128 characters long"),
            ChatToolCallConstraints::ToolInputSize => {
                ErrorKind::BadRequest.with_message("Tool input size exceeds maximum allowed")
            }
            ChatToolCallConstraints::ToolOutputSize => {
                ErrorKind::BadRequest.with_message("Tool output size exceeds maximum allowed")
            }
            ChatToolCallConstraints::CompletedAfterStarted => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("chat_tool_call")
    }
}

impl From<ChatOperationConstraints> for Error<'static> {
    fn from(c: ChatOperationConstraints) -> Self {
        let error = match c {
            ChatOperationConstraints::OperationTypeLength => ErrorKind::BadRequest
                .with_message("Operation type must be between 1 and 64 characters long"),
            ChatOperationConstraints::OperationDiffSize => {
                ErrorKind::BadRequest.with_message("Operation diff size exceeds maximum allowed")
            }
            ChatOperationConstraints::RevertRequiresApplied => ErrorKind::BadRequest
                .with_message("Cannot revert an operation that has not been applied"),
            ChatOperationConstraints::AppliedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("chat_operation")
    }
}
