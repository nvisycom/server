//! Workspace connections table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace connections table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceConnectionConstraints {
    // Name validation constraints
    #[strum(serialize = "workspace_connections_name_length")]
    NameLength,

    // Provider validation constraints
    #[strum(serialize = "workspace_connections_provider_length")]
    ProviderLength,

    // Data validation constraints
    #[strum(serialize = "workspace_connections_data_size")]
    DataSize,

    // Uniqueness constraints
    #[strum(serialize = "workspace_connections_name_unique_idx")]
    NameUnique,

    // Chronological constraints
    #[strum(serialize = "workspace_connections_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_connections_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspaceConnectionConstraints {
    /// Creates a new [`WorkspaceConnectionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceConnectionConstraints::NameLength
            | WorkspaceConnectionConstraints::ProviderLength
            | WorkspaceConnectionConstraints::DataSize => ConstraintCategory::Validation,

            WorkspaceConnectionConstraints::NameUnique => ConstraintCategory::Uniqueness,

            WorkspaceConnectionConstraints::UpdatedAfterCreated
            | WorkspaceConnectionConstraints::DeletedAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspaceConnectionConstraints> for String {
    #[inline]
    fn from(val: WorkspaceConnectionConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceConnectionConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
