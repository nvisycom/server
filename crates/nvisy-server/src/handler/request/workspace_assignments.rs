//! Assignment request types (assign a reviewer, change status, filter).

use garde::Validate;
use nvisy_postgres::types::{AssignmentFilter, AssignmentStatus, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters addressing one assignment by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentPathParams {
    /// Unique identifier of the assignment.
    pub assignment_id: Uuid,
}

/// Request payload to assign a file to a reviewer.
///
/// A file may be assigned to several reviewers at once; assigning the same
/// reviewer twice is a no-op. Requires `AssignTasks`.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateAssignment {
    /// Handle of the workspace member to assign the file to.
    pub assignee: Handle,
}

/// Request payload to change an assignment's review status.
///
/// Allowed for the assignee (their own review status) or a member with
/// `AssignTasks`.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateAssignment {
    /// The new review status.
    pub status: AssignmentStatus,
}

/// Query parameters for listing a workspace's assignments.
///
/// Every field is an optional filter; unset fields impose no constraint. The
/// special assignee value `me` resolves to the caller's own account and is
/// handled by the handler, not carried here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAssignmentsQuery {
    /// Filter by the reviewer the file is assigned to (a member handle, or the
    /// literal `me` for the caller).
    pub assignee: Option<String>,
    /// Filter by review status.
    pub status: Option<AssignmentStatus>,
    /// Filter by the file under review.
    pub file_id: Option<Uuid>,
}

impl WorkspaceAssignmentsQuery {
    /// Builds the repository filter, given the already-resolved assignee account
    /// id (the handler resolves `me` / a handle to an id, or `None` for no
    /// assignee filter).
    pub fn into_filter(self, assignee_account_id: Option<Uuid>) -> AssignmentFilter {
        AssignmentFilter {
            assignee_account_id,
            status: self.status,
            file_id: self.file_id,
        }
    }
}
