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

/// Request payload to trigger a connection sync.
///
/// The direction is determined by the connection's configured `sync_mode`.
/// - Import connections need no body: the sync fetches every not-yet-imported
///   object under the connection's root path.
/// - Export connections push one workspace file (`file_id`) to one object
///   `key`; both are required for export and ignored for import.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncConnection {
    /// The workspace file to export (export connections only).
    pub file_id: Option<Uuid>,
    /// The destination object key for an export, relative to the root path.
    #[validate(length(min = 1, max = 1024))]
    pub key: Option<String>,
}
