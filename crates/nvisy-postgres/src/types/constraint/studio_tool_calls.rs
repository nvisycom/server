//! Studio tool calls table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Studio tool calls table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum StudioToolCallConstraints {
    // Tool call validation constraints
    #[strum(serialize = "studio_tool_calls_tool_name_length")]
    ToolNameLength,
    #[strum(serialize = "studio_tool_calls_tool_input_size")]
    ToolInputSize,
    #[strum(serialize = "studio_tool_calls_tool_output_size")]
    ToolOutputSize,

    // Tool call chronological constraints
    #[strum(serialize = "studio_tool_calls_completed_after_started")]
    CompletedAfterStarted,
}

impl StudioToolCallConstraints {
    /// Creates a new [`StudioToolCallConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            StudioToolCallConstraints::ToolNameLength
            | StudioToolCallConstraints::ToolInputSize
            | StudioToolCallConstraints::ToolOutputSize => ConstraintCategory::Validation,

            StudioToolCallConstraints::CompletedAfterStarted => ConstraintCategory::Chronological,
        }
    }
}

impl From<StudioToolCallConstraints> for String {
    #[inline]
    fn from(val: StudioToolCallConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for StudioToolCallConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
