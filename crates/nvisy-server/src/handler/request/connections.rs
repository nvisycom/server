//! Connection request types.

use nvisy_postgres::types::{ConnectionId, SyncMode};
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
    /// Object store provider (`s3`, `azure`, `gcs`).
    #[validate(length(min = 1, max = 64))]
    pub provider: String,
    /// Whether the connection imports data in or exports data out.
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; omit for manual-only.
    #[validate(length(min = 9, max = 100))]
    pub schedule_cron: Option<String>,
    /// Connection data to be encrypted (credentials + context).
    /// The structure depends on the provider type.
    pub data: serde_json::Value,
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
    /// Cron expression for scheduled imports; send `null` to clear it.
    #[validate(length(min = 9, max = 100))]
    pub schedule_cron: Option<String>,
    /// Connection data to be encrypted (credentials + context).
    /// If provided, replaces the existing encrypted data.
    pub data: Option<serde_json::Value>,
}

/// Query parameters for listing connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsQuery {
    /// Filter by provider type.
    pub provider: Option<String>,
}
