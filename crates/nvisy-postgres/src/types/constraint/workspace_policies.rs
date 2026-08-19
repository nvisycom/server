//! Workspace policies table constraint violations.

use strum::EnumString;

/// Workspace policies table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
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
