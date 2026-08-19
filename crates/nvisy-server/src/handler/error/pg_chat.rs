//! Chat-related constraint violation error handlers.

use nvisy_postgres::types::{ChatMessageConstraints, ChatSessionConstraints};

use crate::handler::{Error, ErrorKind};

impl From<ChatSessionConstraints> for Error<'static> {
    fn from(c: ChatSessionConstraints) -> Self {
        let error = match c {
            ChatSessionConstraints::TitleLength => ErrorKind::BadRequest
                .with_message("Chat title must be between 1 and 255 characters"),
        };
        error.with_resource("chat_session")
    }
}

impl From<ChatMessageConstraints> for Error<'static> {
    fn from(c: ChatMessageConstraints) -> Self {
        let error = match c {
            ChatMessageConstraints::ContentSize => {
                ErrorKind::BadRequest.with_message("Chat message is empty or too large")
            }
            // A parent must be in the same session; a caller supplying a
            // cross-session parent is a bad request.
            ChatMessageConstraints::IdSession | ChatMessageConstraints::Parent => {
                ErrorKind::BadRequest.with_message("Parent message does not belong to this session")
            }
        };
        error.with_resource("chat_message")
    }
}
