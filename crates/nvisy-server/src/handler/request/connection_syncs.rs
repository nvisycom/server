//! Connection sync request types.

use nvisy_postgres::types::ConnectionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

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

/// Direction of a connection sync.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SyncDirection {
    /// Pull an object from the connection into the workspace file store.
    Import,
    /// Push a workspace file out to the connection.
    Export,
}

/// Request payload to trigger a connection sync.
///
/// A sync transfers a single object: import pulls one `key` from the
/// connection; export pushes one `file_id` to one `key`. Prefix/batch syncs
/// are not supported yet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncConnection {
    /// Whether to import from or export to the connection.
    pub direction: SyncDirection,
    /// A single object key within the connection, relative to its root path.
    #[validate(length(min = 1, max = 1024))]
    pub key: String,
    /// The workspace file to export. Required for `export`, ignored for `import`.
    pub file_id: Option<Uuid>,
}
