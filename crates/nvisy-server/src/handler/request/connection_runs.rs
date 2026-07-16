//! Connection sync run request types.

use nvisy_postgres::types::SyncStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters for connection sync run operations.
///
/// The workspace is resolved separately from the `{workspaceSlug}` segment by
/// the [`WorkspaceContext`] extractor.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRunPathParams {
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
