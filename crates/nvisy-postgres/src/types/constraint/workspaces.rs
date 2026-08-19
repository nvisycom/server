//! Workspaces table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
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

impl WorkspaceConstraints {
    /// Creates a new [`WorkspaceConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<WorkspaceConstraints> for String {
    #[inline]
    fn from(val: WorkspaceConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
