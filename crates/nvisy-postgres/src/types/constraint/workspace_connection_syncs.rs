//! Workspace connection syncs table constraint violations.

use strum::EnumString;

/// Workspace connection syncs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceConnectionSyncConstraints {
    #[strum(serialize = "workspace_connection_syncs_error_message_length")]
    ErrorMessageLength,
    #[strum(serialize = "workspace_connection_syncs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_connection_syncs_one_active_idx")]
    OneActivePerConnection,
}
