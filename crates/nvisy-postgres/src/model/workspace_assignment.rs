//! Workspace assignment model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_assignments;
use crate::types::AssignmentStatus;

/// An assignment: one reviewer's assignment of one file for redaction review.
///
/// A file may be assigned to several reviewers at once (like GitHub assignees);
/// each reviewer's assignment is its own row with its own [`AssignmentStatus`].
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_assignments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceAssignment {
    /// Unique assignment identifier.
    pub id: Uuid,
    /// Workspace this assignment belongs to (denormalized for fast per-workspace
    /// queries).
    pub workspace_id: Uuid,
    /// File under review.
    pub file_id: Uuid,
    /// Reviewer the file is assigned to.
    pub assignee_account_id: Uuid,
    /// Account that created the assignment, for the audit trail. `None` if that
    /// account was since removed.
    pub assigned_account_id: Option<Uuid>,
    /// The reviewer's current review status for this file.
    pub status: AssignmentStatus,
    /// When the assignment was created.
    pub created_at: Timestamp,
    /// When the assignment was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new workspace assignment.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_assignments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceAssignment {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// File ID (required).
    pub file_id: Uuid,
    /// Reviewer the file is assigned to (required).
    pub assignee_account_id: Uuid,
    /// Account that created the assignment (the assigner).
    pub assigned_account_id: Option<Uuid>,
    /// Initial status.
    pub status: Option<AssignmentStatus>,
}

impl NewWorkspaceAssignment {
    /// A minimal assignment of `file_id` to `assignee_account_id`, for tests.
    /// The status takes its database default (`assigned`).
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, file_id: Uuid, assignee_account_id: Uuid) -> Self {
        Self {
            workspace_id,
            file_id,
            assignee_account_id,
            ..Default::default()
        }
    }
}

/// Data for updating a workspace assignment.
///
/// Only the review status is mutable: reassignment is remove-then-add, not a
/// field change.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_assignments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceAssignment {
    /// The reviewer's review status for this file.
    pub status: Option<AssignmentStatus>,
}
