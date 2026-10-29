//! The provider-agnostic conversation types callers build history from.
//!
//! These wrap the underlying rig message model so consumers depend on this
//! crate's own types rather than `rig`'s.

use rig::completion::Message;

/// Who authored a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A system instruction.
    System,
    /// A message from the user.
    User,
    /// A reply from the assistant.
    Assistant,
}

/// One turn of a conversation: a role and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurn {
    /// Who authored the turn.
    pub role: Role,
    /// The turn's text.
    pub content: String,
}

impl ChatTurn {
    /// A system turn.
    pub fn system(content: impl Into<String>) -> Self {
        Self::of(Role::System, content)
    }

    /// A user turn.
    pub fn user(content: impl Into<String>) -> Self {
        Self::of(Role::User, content)
    }

    /// An assistant turn.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::of(Role::Assistant, content)
    }

    fn of(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

impl From<ChatTurn> for Message {
    fn from(turn: ChatTurn) -> Self {
        match turn.role {
            Role::System => Message::system(turn.content),
            Role::User => Message::user(turn.content),
            Role::Assistant => Message::assistant(turn.content),
        }
    }
}
