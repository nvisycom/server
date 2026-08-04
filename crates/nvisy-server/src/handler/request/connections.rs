//! Connection request types.

use nvisy_object::providers::ConnectionConfig;
use nvisy_postgres::types::{ConnectionId, SyncDeletionPolicy, SyncMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Path parameters for connection operations.
///
/// The workspace is resolved separately from the `{workspaceSlug}` segment by
/// the [`WorkspaceContext`] extractor.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPathParams {
    /// Opaque identifier of the connection.
    pub connection_id: ConnectionId,
}

/// Request payload for creating a new workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnection {
    /// Human-readable connection display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
    /// Whether the connection imports data in or exports data out.
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; omit for manual-only.
    #[validate(length(min = 9, max = 100))]
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    #[serde(default)]
    pub deletion_policy: SyncDeletionPolicy,
    /// Whether the connection is enabled for syncing. Omit to default to active;
    /// set `false` to create it disabled.
    pub is_active: Option<bool>,
    /// Typed provider configuration (provider tag + its credentials + optional
    /// root path), encrypted at rest. The `provider` tag selects which
    /// credential shape is required.
    pub config: ConnectionConfig,
}

/// Request payload for updating an existing workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnection {
    /// Human-readable connection display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Whether the connection imports data in or exports data out.
    pub sync_mode: Option<SyncMode>,
    /// Cron expression for scheduled imports. Omit to leave unchanged; send
    /// `null` to clear it (make the connection manual-only).
    #[validate(length(min = 9, max = 100))]
    pub schedule_cron: Option<Option<String>>,
    /// How an import reconciles files whose source object was deleted.
    pub deletion_policy: Option<SyncDeletionPolicy>,
    /// Whether the connection is enabled for syncing. `false` disables it
    /// (pausing scheduled syncs and rejecting manual ones); omit to leave
    /// unchanged.
    pub is_active: Option<bool>,
    /// Typed provider configuration. If provided, fully replaces the stored
    /// config (and, with it, the provider). Omit to leave it unchanged.
    pub config: Option<ConnectionConfig>,
}

/// Query parameters for listing connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsQuery {
    /// Filter by provider (`s3`, `azure`, `gcs`). Repeatable; a connection
    /// matches if it uses any of the given providers. Empty means no filter.
    #[serde(default)]
    pub provider: Vec<String>,
}
