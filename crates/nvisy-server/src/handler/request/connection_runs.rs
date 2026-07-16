//! Connection sync run request types.

use nvisy_postgres::types::SyncStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// URL slug of the connection the run belongs to.
    pub connection_slug: String,
    /// Per-connection sequential run number.
    pub run_number: i32,
}

/// Query parameters for listing connection sync runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRunsQuery {
    /// Filter by run status.
    pub status: Option<SyncStatus>,
}
