//! Workspace policies table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace policies table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePolicyConstraints {
    #[strum(serialize = "workspace_policies_slug_length")]
    SlugLength,
    #[strum(serialize = "workspace_policies_slug_format")]
    SlugFormat,
    #[strum(serialize = "workspace_policies_display_name_length")]
    NameLength,
    #[strum(serialize = "workspace_policies_description_length")]
    DescriptionLength,
    #[strum(serialize = "workspace_policies_definition_size")]
    DefinitionSize,
    #[strum(serialize = "workspace_policies_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_policies_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_policies_slug_unique_idx")]
    SlugUnique,
    #[strum(serialize = "workspace_policies_display_name_unique_idx")]
    NameUnique,
}

impl WorkspacePolicyConstraints {
    /// Creates a new [`WorkspacePolicyConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
