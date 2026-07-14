//! Workspace policies table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace policies table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePolicyConstraints {
    // Validation constraints
    #[strum(serialize = "workspace_policies_name_length")]
    NameLength,
    #[strum(serialize = "workspace_policies_description_length")]
    DescriptionLength,
    #[strum(serialize = "workspace_policies_version_length")]
    VersionLength,
    #[strum(serialize = "workspace_policies_definition_size")]
    DefinitionSize,
    #[strum(serialize = "workspace_policies_metadata_size")]
    MetadataSize,

    // Uniqueness constraints
    #[strum(serialize = "workspace_policies_workspace_id_id_key")]
    WorkspaceIdIdUnique,

    // Chronological constraints
    #[strum(serialize = "workspace_policies_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_policies_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspacePolicyConstraints {
    /// Creates a new [`WorkspacePolicyConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspacePolicyConstraints::NameLength
            | WorkspacePolicyConstraints::DescriptionLength
            | WorkspacePolicyConstraints::VersionLength
            | WorkspacePolicyConstraints::DefinitionSize
            | WorkspacePolicyConstraints::MetadataSize => ConstraintCategory::Validation,

            WorkspacePolicyConstraints::WorkspaceIdIdUnique => ConstraintCategory::Uniqueness,

            WorkspacePolicyConstraints::UpdatedAfterCreated
            | WorkspacePolicyConstraints::DeletedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspacePolicyConstraints> for String {
    #[inline]
    fn from(val: WorkspacePolicyConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspacePolicyConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
