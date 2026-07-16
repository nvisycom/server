//! Workspace contexts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace contexts table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceContextConstraints {
    // Slug validation constraints
    #[strum(serialize = "workspace_contexts_slug_length")]
    SlugLength,
    #[strum(serialize = "workspace_contexts_slug_format")]
    SlugFormat,

    // Name validation constraints
    #[strum(serialize = "workspace_contexts_name_length")]
    NameLength,

    // Description validation constraints
    #[strum(serialize = "workspace_contexts_description_length")]
    DescriptionLength,

    // Definition validation constraints
    #[strum(serialize = "workspace_contexts_definition_size")]
    DefinitionSize,

    // Version validation constraints
    #[strum(serialize = "workspace_contexts_version_length")]
    VersionLength,

    // Metadata validation constraints
    #[strum(serialize = "workspace_contexts_metadata_size")]
    MetadataSize,

    // Uniqueness constraints
    #[strum(serialize = "workspace_contexts_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_contexts_workspace_id_slug_key")]
    SlugUnique,

    // Chronological constraints
    #[strum(serialize = "workspace_contexts_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_contexts_deleted_after_created")]
    DeletedAfterCreated,
}

impl WorkspaceContextConstraints {
    /// Creates a new [`WorkspaceContextConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceContextConstraints::SlugLength
            | WorkspaceContextConstraints::SlugFormat
            | WorkspaceContextConstraints::NameLength
            | WorkspaceContextConstraints::DescriptionLength
            | WorkspaceContextConstraints::DefinitionSize
            | WorkspaceContextConstraints::VersionLength
            | WorkspaceContextConstraints::MetadataSize => ConstraintCategory::Validation,

            WorkspaceContextConstraints::WorkspaceIdIdUnique
            | WorkspaceContextConstraints::SlugUnique => ConstraintCategory::Uniqueness,

            WorkspaceContextConstraints::UpdatedAfterCreated
            | WorkspaceContextConstraints::DeletedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspaceContextConstraints> for String {
    #[inline]
    fn from(val: WorkspaceContextConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceContextConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
