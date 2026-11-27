//! Workspace providers table constraint violations.

use strum::EnumString;

/// Workspace providers table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceProviderConstraints {
    #[strum(serialize = "workspace_providers_display_name_length")]
    NameLength,
    #[strum(serialize = "workspace_providers_provider_length")]
    ProviderLength,
    #[strum(serialize = "workspace_providers_data_size")]
    DataSize,
    #[strum(serialize = "workspace_providers_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_providers_display_name_unique_idx")]
    NameUnique,
}
