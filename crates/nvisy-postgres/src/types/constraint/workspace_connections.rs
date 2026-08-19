//! Workspace connections table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace connections table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceConnectionConstraints {
    #[strum(serialize = "workspace_connections_display_name_length")]
    NameLength,
    #[strum(serialize = "workspace_connections_provider_length")]
    ProviderLength,
    #[strum(serialize = "workspace_connections_data_size")]
    DataSize,
    #[strum(serialize = "workspace_connections_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_connection_schedule_cron_length")]
    ScheduleCronLength,
    #[strum(serialize = "workspace_connection_schedule_import_only")]
    ScheduleImportOnly,
    #[strum(serialize = "workspace_connections_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_connections_display_name_unique_idx")]
    NameUnique,
}

impl WorkspaceConnectionConstraints {
    /// Creates a new [`WorkspaceConnectionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
