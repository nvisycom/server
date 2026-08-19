//! Chat sessions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Chat sessions table constraint violations.
///
/// Enumerates the constraints a client request can trip that map to a specific
/// non-500 response. Server-controlled invariants (ownership and active-leaf
/// foreign keys, timestamp ordering) fall through to the generic handler.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ChatSessionConstraints {
    // Validation constraints
    #[strum(serialize = "chat_sessions_title_length")]
    TitleLength,
}

impl ChatSessionConstraints {
    /// Creates a new [`ChatSessionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<ChatSessionConstraints> for String {
    #[inline]
    fn from(val: ChatSessionConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ChatSessionConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
