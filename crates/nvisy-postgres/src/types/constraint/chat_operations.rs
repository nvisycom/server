//! Chat operations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Chat operations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ChatOperationConstraints {
    // Operation validation constraints
    #[strum(serialize = "chat_operations_operation_type_length")]
    OperationTypeLength,
    #[strum(serialize = "chat_operations_operation_diff_size")]
    OperationDiffSize,

    // Operation business logic constraints
    #[strum(serialize = "chat_operations_revert_requires_applied")]
    RevertRequiresApplied,

    // Operation chronological constraints
    #[strum(serialize = "chat_operations_applied_after_created")]
    AppliedAfterCreated,
}

impl ChatOperationConstraints {
    /// Creates a new [`ChatOperationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ChatOperationConstraints::OperationTypeLength
            | ChatOperationConstraints::OperationDiffSize => ConstraintCategory::Validation,

            ChatOperationConstraints::RevertRequiresApplied => ConstraintCategory::BusinessLogic,

            ChatOperationConstraints::AppliedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<ChatOperationConstraints> for String {
    #[inline]
    fn from(val: ChatOperationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ChatOperationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
