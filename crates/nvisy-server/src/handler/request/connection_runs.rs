//! Connection sync run request types.

use nvisy_postgres::types::SyncStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters for connection sync run operations.
///
/// Since run IDs are globally unique UUIDs, the connection context can be
/// derived from the run record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRunPathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the connection sync run.
    pub run_id: Uuid,
}

/// Query parameters for listing connection sync runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRunsQuery {
    /// Filter by run status.
    pub status: Option<SyncStatus>,
}
