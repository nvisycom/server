//! Workspace connections table constraint violations.

use strum::EnumString;

/// Workspace connections table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
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
