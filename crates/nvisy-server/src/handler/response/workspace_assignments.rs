//! Assignment response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceAssignment as AssignmentModel;
use nvisy_postgres::types::AssignmentStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a file review assignment.
///
/// A file may be assigned to several reviewers at once (like GitHub assignees);
/// each assignment is its own resource with its own review status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    /// Unique identifier of the assignment.
    pub id: Uuid,
    /// File under review.
    pub file_id: Uuid,
    /// Display name of the file under review, for showing the assignment without
    /// a separate file lookup. `None` if the file was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// Reviewer the file is assigned to.
    pub assignee: AccountRef,
    /// The reviewer's current review status for this file.
    pub status: AssignmentStatus,
    /// When the assignment was created.
    pub created_at: Timestamp,
    /// When the assignment was last updated.
    pub updated_at: Timestamp,
}

/// Paginated response for assignments.
pub type AssignmentsPage = Page<Assignment>;

impl Assignment {
    /// Creates an assignment response from the database model, the resolved
    /// reviewer reference, and the file display name.
    pub fn from_model(
        assignment: AssignmentModel,
        assignee: AccountRef,
        file_name: Option<String>,
    ) -> Self {
        Self {
            id: assignment.id,
            file_id: assignment.file_id,
            file_name,
            assignee,
            status: assignment.status,
            created_at: assignment.created_at.into(),
            updated_at: assignment.updated_at.into(),
        }
    }
}
