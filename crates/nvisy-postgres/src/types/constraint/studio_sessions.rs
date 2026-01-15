//! Studio sessions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Studio sessions table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum StudioSessionConstraints {
    // Session validation constraints
    #[strum(serialize = "studio_sessions_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "studio_sessions_model_config_size")]
    ModelConfigSize,
    #[strum(serialize = "studio_sessions_message_count_min")]
    MessageCountMin,
    #[strum(serialize = "studio_sessions_token_count_min")]
    TokenCountMin,

    // Session chronological constraints
    #[strum(serialize = "studio_sessions_updated_after_created")]
    UpdatedAfterCreated,
}

impl StudioSessionConstraints {
    /// Creates a new [`StudioSessionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            StudioSessionConstraints::DisplayNameLength
            | StudioSessionConstraints::ModelConfigSize
            | StudioSessionConstraints::MessageCountMin
            | StudioSessionConstraints::TokenCountMin => ConstraintCategory::Validation,

            StudioSessionConstraints::UpdatedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<StudioSessionConstraints> for String {
    #[inline]
    fn from(val: StudioSessionConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for StudioSessionConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
