//! Connection request types.

use garde::Validate;
use nvisy_file_service::provider::FileServiceProvider;
use nvisy_postgres::types::{ConnectionId, SyncDeletionPolicy, SyncMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::extract::validators::{validate_non_blank, validate_non_blank_opt};
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

/// Body for minting a browser file-picker token.
///
/// The OneDrive v8 picker requests a token per resource (it names the resource in
/// each `authenticate` command); the caller passes that `resource` so the server
/// mints a token scoped to exactly it. Ignored by providers whose picker takes a
/// single provider token (Google Drive, Box); omit it for those.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct PickerTokenRequest {
    /// The resource the picker asked for (its `authenticate` command's
    /// `resource`), e.g. `https://contoso-my.sharepoint.com`. Optional; when
    /// absent the server uses the connection's default picker resource.
    #[garde(length(chars, min = 1, max = 2048))]
    pub resource: Option<String>,
}

/// Scheduled-sync configuration for a schedulable connection.
///
/// Accepted only for providers that can sync on a timer (object stores); rejected
/// for others — a file service transfers on demand (picker import, per-file
/// export), and an LLM does not transfer at all. Omit for on-demand only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct SyncScheduleInput {
    /// Whether the connection imports data in or exports data out. Required: the
    /// caller states the direction explicitly rather than defaulting to one.
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; omit for manual-only.
    #[garde(length(chars, min = 9, max = 100))]
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    #[serde(default)]
    pub deletion_policy: SyncDeletionPolicy,
}

/// Request payload for creating a new workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateConnection {
    /// Human-readable connection display name.
    #[garde(length(chars, min = 1, max = 255), custom(validate_non_blank))]
    pub display_name: String,
    /// Whether the connection is enabled. Omit to default to active; set `false`
    /// to create it disabled.
    pub is_active: Option<bool>,
    /// Typed provider configuration (provider tag + its credentials), encrypted
    /// at rest. The `provider` tag selects which credential shape is required and
    /// which capability the connection has.
    pub config: ConnectionConfig,
    /// Scheduled-sync configuration. Accepted only for schedulable providers
    /// (object stores); rejected for others. Omit for on-demand only.
    #[garde(dive)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncScheduleInput>,
}

/// Path parameters for the OAuth start endpoint: which cloud file provider to
/// begin authorizing. The provider is the crate's [`FileServiceProvider`], so the API and
/// stored config name each provider identically.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartPathParams {
    /// The cloud file provider to connect.
    pub provider: FileServiceProvider,
}

/// Request payload for starting a cloud file-service OAuth authorization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct StartFileServiceOAuth {
    /// Human-readable name for the connection to be created on success.
    #[garde(length(chars, min = 1, max = 255), custom(validate_non_blank))]
    pub display_name: String,
    /// Where to scope the sync: a folder id (Drive, OneDrive, Box) or a folder
    /// path (Dropbox). Omit to use the account root.
    #[garde(length(chars, min = 1, max = 255))]
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
#[garde(allow_unvalidated)]
pub struct UpdateConnection {
    /// Human-readable connection display name.
    #[garde(length(chars, min = 1, max = 255), custom(validate_non_blank_opt))]
    pub display_name: Option<String>,
    /// Whether the connection is enabled. `false` disables it (pausing scheduled
    /// syncs and rejecting manual ones); omit to leave unchanged.
    pub is_active: Option<bool>,
    /// Typed provider configuration. If provided, fully replaces the stored
    /// config (and, with it, the provider). Omit to leave it unchanged.
    pub config: Option<ConnectionConfig>,
    /// Scheduled-sync configuration. Accepted only for schedulable providers
    /// (object stores); rejected for others. Omit to leave unchanged.
    #[garde(dive)]
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
