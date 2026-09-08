//! Connection request types.

use nvisy_file_service::provider::Provider;
use nvisy_postgres::types::{ConnectionId, SyncDeletionPolicy, SyncMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

use crate::service::ConnectionConfig;

/// Rejects a value that is empty once trimmed, matching the database's
/// non-empty-trimmed constraint on connection display names.
fn validate_non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }
    Ok(())
}

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

/// Body for minting a browser file-picker token.
///
/// The OneDrive v8 picker requests a token per resource (it names the resource in
/// each `authenticate` command); the caller passes that `resource` so the server
/// mints a token scoped to exactly it. Ignored by providers whose picker takes a
/// single provider token (Google Drive, Box); omit it for those.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PickerTokenRequest {
    /// The resource the picker asked for (its `authenticate` command's
    /// `resource`), e.g. `https://contoso-my.sharepoint.com`. Optional; when
    /// absent the server uses the connection's default picker resource.
    #[validate(length(min = 1, max = 2048))]
    pub resource: Option<String>,
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

/// Path parameters for the OAuth start endpoint: which cloud file provider to
/// begin authorizing. The provider is the crate's [`Provider`], so the API and
/// stored config name each provider identically.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartPathParams {
    /// The cloud file provider to connect.
    pub provider: Provider,
}

/// Request payload for starting a cloud file-service OAuth authorization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StartFileServiceOAuth {
    /// Human-readable name for the connection to be created on success.
    #[validate(length(min = 1, max = 255), custom(function = "validate_non_blank"))]
    pub display_name: String,
    /// Where to scope the sync: a folder id (Drive, OneDrive, Box) or a folder
    /// path (Dropbox). Omit to use the account root.
    #[validate(length(min = 1, max = 255))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

/// Query parameters the provider appends when redirecting to the OAuth callback.
///
/// On success the provider sends `code` + `state`; on denial it sends `error`
/// (and `state`) with no `code`, so `code` is optional and the handler treats a
/// missing code or a present error as a failed authorization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCallbackQuery {
    /// The authorization code to exchange for tokens; absent on a denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The opaque CSRF state echoed back; must match a pending authorization.
    pub state: String,
    /// The provider's error code when the user denied or the flow failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
