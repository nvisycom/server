//! Chat messages table constraint violations.

use strum::EnumString;

/// Chat messages table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum ChatMessageConstraints {
    #[strum(serialize = "chat_messages_content_size")]
    ContentSize,

    // Tree integrity: a parent must be in the same session.
    #[strum(serialize = "chat_messages_id_session_key")]
    IdSession,
    #[strum(serialize = "chat_messages_parent_fkey")]
    Parent,
}
