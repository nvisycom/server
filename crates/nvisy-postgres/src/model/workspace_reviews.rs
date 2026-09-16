//! Workspace document-review model: an optional, purpose-scoped sign-off effort on
//! a document. A document has 0..N reviews. A review owns a discussion
//! [`WorkspaceThread`] and carries its [`ReviewStatus`] and an optional free-text
//! `purpose`. It references its assignees, detections, and redactions via the link
//! models ([`NewReviewAssignee`](super::NewReviewAssignee) et al.).
//!
//! [`WorkspaceThread`]: super::WorkspaceThread

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_reviews;
use crate::types::ReviewStatus;

/// A document's purpose-scoped review: it owns a discussion [`WorkspaceThread`] and
/// carries its [`ReviewStatus`] and an optional `purpose` label. Its assignees live
/// in a link table.
///
/// [`WorkspaceThread`]: super::WorkspaceThread
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceReview {
    /// Unique review identifier.
    pub id: Uuid,
    /// Workspace this review belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Document under review (a document may have many reviews).
    pub document_id: Uuid,
    /// Discussion thread this review owns (one review per thread).
    pub thread_id: Uuid,
    /// Optional free-text label for the review's purpose/audience.
    pub purpose: Option<String>,
    /// Review state.
    pub review_status: ReviewStatus,
    /// When the review was created.
    pub created_at: Timestamp,
    /// When the review was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new document review.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceReview {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Document under review (required).
    pub document_id: Uuid,
    /// The discussion thread this review owns (required).
    pub thread_id: Uuid,
    /// Optional purpose label.
    pub purpose: Option<String>,
}

/// Data for updating a review's mutable fields.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceReview {
    /// The new review status; `None` leaves it unchanged.
    pub review_status: Option<ReviewStatus>,
}
