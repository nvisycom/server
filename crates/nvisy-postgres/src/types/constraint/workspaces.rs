//! Workspaces table constraint violations.

use strum::EnumString;

/// Workspace table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceConstraints {
    #[strum(serialize = "workspaces_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "workspaces_slug_length")]
    SlugLength,
    #[strum(serialize = "workspaces_slug_format")]
    SlugFormat,
    #[strum(serialize = "workspaces_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "workspaces_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspaces_settings_size")]
    SettingsSize,
    #[strum(serialize = "workspaces_slug_unique_idx")]
    SlugUnique,
    #[strum(serialize = "workspaces_display_name_owner_unique_idx")]
    NameUnique,
}
