//! Connection request types.

use nvisy_postgres::types::{ConnectionId, SyncDeletionPolicy, SyncMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::service::ConnectionConfig;

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

/// Sync configuration for a sync-capable connection (object stores).
///
/// Only meaningful for connections whose provider supports syncing; omitted for
/// connections that do not (e.g. LLM inference).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncScheduleInput {
    /// Whether the connection imports data in or exports data out.
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; omit for manual-only.
    #[validate(length(min = 9, max = 100))]
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    #[serde(default)]
    pub deletion_policy: SyncDeletionPolicy,
}

/// Request payload for creating a new workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnection {
    /// Human-readable connection display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
    /// Whether the connection is enabled. Omit to default to active; set `false`
    /// to create it disabled.
    pub is_active: Option<bool>,
    /// Typed provider configuration (provider tag + its credentials), encrypted
    /// at rest. The `provider` tag selects which credential shape is required and
    /// which capability the connection has.
    pub config: ConnectionConfig,
    /// Sync configuration. Applies only to sync-capable providers (object
    /// stores); rejected for others. Omit for manual-only defaults.
    #[validate(nested)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncScheduleInput>,
}

/// A cloud file-service provider that connects over OAuth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CloudFilesProvider {
    /// Google Drive.
    GoogleDrive,
    /// Dropbox.
    Dropbox,
    /// OneDrive (Microsoft Graph).
    OneDrive,
    /// Box.
    Box,
}

/// Path parameters for the OAuth start endpoint: which cloud file provider to
/// begin authorizing.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartPathParams {
    /// The cloud file provider to connect.
    pub provider: CloudFilesProvider,
}

/// Request payload for starting a cloud file-service OAuth authorization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StartCloudFilesOAuth {
    /// Human-readable name for the connection to be created on success.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
    /// Where to scope the sync: a folder id (Drive, OneDrive, Box) or a folder
    /// path (Dropbox). Omit to use the account root.
    #[validate(length(min = 1, max = 255))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

/// Query parameters the provider appends when redirecting to the OAuth callback.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCallbackQuery {
    /// The authorization code to exchange for tokens.
    pub code: String,
    /// The opaque CSRF state echoed back; must match a pending authorization.
    pub state: String,
}

/// Request payload for updating an existing workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnection {
    /// Human-readable connection display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Whether the connection is enabled. `false` disables it (pausing scheduled
    /// syncs and rejecting manual ones); omit to leave unchanged.
    pub is_active: Option<bool>,
    /// Typed provider configuration. If provided, fully replaces the stored
    /// config (and, with it, the provider). Omit to leave it unchanged.
    pub config: Option<ConnectionConfig>,
    /// Sync configuration. Applies only to sync-capable providers. Omit to leave
    /// unchanged.
    #[validate(nested)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncScheduleInput>,
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
