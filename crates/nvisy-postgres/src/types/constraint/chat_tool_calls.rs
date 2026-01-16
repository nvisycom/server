//! Chat tool calls table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Chat tool calls table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ChatToolCallConstraints {
    // Tool call validation constraints
    #[strum(serialize = "chat_tool_calls_tool_name_length")]
    ToolNameLength,
    #[strum(serialize = "chat_tool_calls_tool_input_size")]
    ToolInputSize,
    #[strum(serialize = "chat_tool_calls_tool_output_size")]
    ToolOutputSize,

    // Tool call chronological constraints
    #[strum(serialize = "chat_tool_calls_completed_after_started")]
    CompletedAfterStarted,
}

impl ChatToolCallConstraints {
    /// Creates a new [`ChatToolCallConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ChatToolCallConstraints::ToolNameLength
            | ChatToolCallConstraints::ToolInputSize
            | ChatToolCallConstraints::ToolOutputSize => ConstraintCategory::Validation,

            ChatToolCallConstraints::CompletedAfterStarted => ConstraintCategory::Chronological,
        }
    }
}

impl From<ChatToolCallConstraints> for String {
    #[inline]
    fn from(val: ChatToolCallConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ChatToolCallConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
