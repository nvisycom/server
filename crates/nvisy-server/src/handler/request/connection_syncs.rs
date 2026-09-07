//! Connection sync request types.

use nvisy_postgres::types::{ConnectionId, SyncStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Query parameters for listing all syncs across a workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSyncsQuery {
    /// Filter by sync status.
    pub status: Option<SyncStatus>,
    /// Filter by connection provider (`s3`, `azure`, `gcs`). Repeatable; a sync
    /// matches if its connection uses any of the given providers. Empty means no
    /// provider filter.
    #[serde(default)]
    pub provider: Vec<String>,
}

/// Path parameters for a specific connection sync.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncPathParams {
    /// Opaque identifier of the connection.
    pub connection_id: ConnectionId,
    /// Unique identifier of the sync run.
    pub sync_id: Uuid,
}

/// One file the user selected in the provider's picker.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PickedFile {
    /// The provider's file identifier (used to fetch the bytes).
    #[validate(length(min = 1, max = 1024))]
    pub id: String,
    /// The file's display name, as the picker reported it.
    #[validate(length(min = 1, max = 1024))]
    pub name: String,
}

/// Request payload to import a caller-selected set of files from a file-service
/// connection (the provider picker returns id + name per file).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ImportFiles {
    /// The files to import. Already-imported files are skipped.
    #[validate(length(min = 1, max = 500), nested)]
    pub files: Vec<PickedFile>,
}

/// Request payload to export a workspace file back to a connection. The redacted
/// output is written as a new provider file, never overwriting the source.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ExportFile {
    /// The name for the new provider file. Omit to derive it from the workspace
    /// file's display name.
    #[validate(length(min = 1, max = 1024))]
    pub name: Option<String>,
}
