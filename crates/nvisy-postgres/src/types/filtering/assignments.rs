//! Filtering options for assignment queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::AssignmentStatus;

/// Filter options for workspace assignments.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AssignmentFilter {
    /// Filter by the reviewer the file is assigned to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_account_id: Option<Uuid>,
    /// Filter by review status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<AssignmentStatus>,
    /// Filter by the file under review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<Uuid>,
}
