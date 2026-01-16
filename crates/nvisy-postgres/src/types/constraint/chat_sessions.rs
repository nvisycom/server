//! Chat sessions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Chat sessions table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ChatSessionConstraints {
    // Session validation constraints
    #[strum(serialize = "chat_sessions_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "chat_sessions_model_config_size")]
    ModelConfigSize,
    #[strum(serialize = "chat_sessions_message_count_min")]
    MessageCountMin,
    #[strum(serialize = "chat_sessions_token_count_min")]
    TokenCountMin,

    // Session chronological constraints
    #[strum(serialize = "chat_sessions_updated_after_created")]
    UpdatedAfterCreated,
}

impl ChatSessionConstraints {
    /// Creates a new [`ChatSessionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ChatSessionConstraints::DisplayNameLength
            | ChatSessionConstraints::ModelConfigSize
            | ChatSessionConstraints::MessageCountMin
            | ChatSessionConstraints::TokenCountMin => ConstraintCategory::Validation,

            ChatSessionConstraints::UpdatedAfterCreated => ConstraintCategory::Chronological,
        }
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
