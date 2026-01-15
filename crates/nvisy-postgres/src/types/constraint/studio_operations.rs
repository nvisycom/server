//! Studio operations table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Studio operations table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum StudioOperationConstraints {
    // Operation validation constraints
    #[strum(serialize = "studio_operations_operation_type_length")]
    OperationTypeLength,
    #[strum(serialize = "studio_operations_operation_diff_size")]
    OperationDiffSize,

    // Operation business logic constraints
    #[strum(serialize = "studio_operations_revert_requires_applied")]
    RevertRequiresApplied,

    // Operation chronological constraints
    #[strum(serialize = "studio_operations_applied_after_created")]
    AppliedAfterCreated,
}

impl StudioOperationConstraints {
    /// Creates a new [`StudioOperationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            StudioOperationConstraints::OperationTypeLength
            | StudioOperationConstraints::OperationDiffSize => ConstraintCategory::Validation,

            StudioOperationConstraints::RevertRequiresApplied => ConstraintCategory::BusinessLogic,

            StudioOperationConstraints::AppliedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<StudioOperationConstraints> for String {
    #[inline]
    fn from(val: StudioOperationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for StudioOperationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
