//! Chat messages table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Chat messages table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ChatMessageConstraints {
    // Validation constraints
    #[strum(serialize = "chat_messages_content_size")]
    ContentSize,

    // Tree integrity: a parent must be in the same session.
    #[strum(serialize = "chat_messages_id_session_key")]
    IdSession,
    #[strum(serialize = "chat_messages_parent_fkey")]
    Parent,
}

impl ChatMessageConstraints {
    /// Creates a new [`ChatMessageConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<ChatMessageConstraints> for String {
    #[inline]
    fn from(val: ChatMessageConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ChatMessageConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
