//! Connection response types.

use jiff::Timestamp;
use nvisy_postgres::model::{WorkspaceConnection, WorkspaceConnectionSchedule};
use nvisy_postgres::types::{ConnectionId, Handle, ProviderType, SyncDeletionPolicy, SyncMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};

/// A connection's sync configuration, present only for sync-capable connections.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncSchedule {
    /// Whether the connection imports data in or exports data out.
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports, if configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    pub deletion_policy: SyncDeletionPolicy,
    /// When the connection last synced successfully, if ever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced: Option<Timestamp>,
}

/// Response type for a workspace connection.
///
/// Note: The encrypted connection data is never exposed in API responses.
/// Only metadata about the connection is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// Opaque identifier of the connection.
    pub id: ConnectionId,
    /// Handle of the workspace this connection belongs to.
    pub workspace_slug: Handle,
    /// Account that created this connection.
    pub created_by: AccountRef,
    /// Human-readable connection display name.
    pub display_name: String,
    /// Provider identifier (`s3`, `azure`, `gcs`, `openai`, `ollama`, ...).
    pub provider: String,
    /// Capability category of the provider (object store, language model, ...).
    pub provider_type: ProviderType,
    /// Whether the connection is enabled.
    pub is_active: bool,
    /// Sync configuration; present only for sync-capable connections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncSchedule>,
    /// When the connection was created.
    pub created_at: Timestamp,
    /// When the connection was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of connections.
pub type ConnectionsPage = Page<Connection>;

/// Result of a connection reachability check.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionVerification {
    /// Whether the backing store was reachable with the stored credentials.
    pub reachable: bool,
    /// Failure reason when not reachable; omitted on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ConnectionVerification {
    /// A successful verification.
    pub fn reachable() -> Self {
        Self {
            reachable: true,
            error: None,
        }
    }

    /// A failed verification carrying the reason.
    pub fn unreachable(error: impl Into<String>) -> Self {
        Self {
            reachable: false,
            error: Some(error.into()),
        }
    }
}

impl Connection {
    /// Creates a response from a database model and its creator.
    pub fn from_model(
        connection: WorkspaceConnection,
        workspace_slug: Handle,
        created_by: AccountRef,
        schedule: Option<WorkspaceConnectionSchedule>,
        last_synced: Option<Timestamp>,
    ) -> Self {
        let sync = schedule.map(|schedule| SyncSchedule {
            sync_mode: schedule.sync_mode,
            schedule_cron: schedule.schedule_cron,
            deletion_policy: schedule.deletion_policy,
            last_synced,
        });
        Self {
            id: ConnectionId::from_uuid(connection.id),
            workspace_slug,
            created_by,
            display_name: connection.display_name,
            provider: connection.provider,
            provider_type: connection.provider_type,
            is_active: connection.is_active,
            sync,
            created_at: connection.created_at.into(),
            updated_at: connection.updated_at.into(),
        }
    }
}
