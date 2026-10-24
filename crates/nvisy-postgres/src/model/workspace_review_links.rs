//! Review assignee model: the many-to-many rows tying a review to its reviewers,
//! each carrying that reviewer's approval (`approved_at`).

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_review_assignees;

/// A link assigning an account as a reviewer of a review. A new assignee starts
/// unapproved (`approved_at` defaults to NULL).
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_review_assignees)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewReviewAssignee {
    /// The review.
    pub review_id: Uuid,
    /// The assigned reviewer.
    pub account_id: Uuid,
}
