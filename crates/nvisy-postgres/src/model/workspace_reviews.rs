//! Workspace review model: a named discussion on a document with an explicit
//! approval lifecycle. A document has 0..N reviews, which may run concurrently (one
//! per audience). A review carries its [`ReviewStatus`], the detection it is
//! currently reviewing and the redaction it has approved (direct columns), owns a
//! stream of [`WorkspaceReviewComment`] messages and [`WorkspaceReviewEvent`]
//! timeline entries, and references its assignees — whose stamped `approved_at`
//! drive sign-off — via [`NewReviewAssignee`](super::NewReviewAssignee).
//!
//! [`WorkspaceReviewComment`]: super::WorkspaceReviewComment
//! [`WorkspaceReviewEvent`]: super::WorkspaceReviewEvent

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_reviews;
use crate::types::ReviewStatus;

/// A review: a named discussion on a document, carrying its [`ReviewStatus`], the
/// detection it is currently reviewing, and the redaction it approved. Its assignees
/// and approvals live in link tables; its comments and timeline events belong to it.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceReview {
    /// Unique review identifier.
    pub id: Uuid,
    /// Workspace this review belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Document under review (a document may have many concurrent reviews).
    pub document_id: Uuid,
    /// The detection currently under review; `None` before one is linked.
    pub detection_id: Option<Uuid>,
    /// The approved redaction output; `None` until the review is approved.
    pub redaction_id: Option<Uuid>,
    /// Account that opened the review.
    pub author_account_id: Uuid,
    /// Human-readable title.
    pub display_name: String,
    /// Review state.
    pub review_status: ReviewStatus,
    /// When the review was created.
    pub created_at: Timestamp,
    /// When the review was last updated.
    pub updated_at: Timestamp,
    /// When the review was soft-deleted; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new review.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceReview {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Document under review (required).
    pub document_id: Uuid,
    /// The detection the review opens against; `None` for a draft without one yet.
    pub detection_id: Option<Uuid>,
    /// Opening author account ID (required).
    pub author_account_id: Uuid,
    /// Title (required).
    pub display_name: String,
}

impl NewWorkspaceReview {
    /// A minimal review of `document_id` opened by `author`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, document_id: Uuid, author_account_id: Uuid) -> Self {
        Self {
            workspace_id,
            document_id,
            detection_id: None,
            author_account_id,
            display_name: "A test review.".to_owned(),
        }
    }
}

/// Data for updating a review's mutable fields.
///
/// The two id fields are doubly optional: the outer `None` leaves the column
/// unchanged, an inner `Some` sets it, and an inner `None` clears it to `NULL` (so a
/// reopen can clear `redaction_id`).
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceReview {
    /// The new title; `None` leaves it unchanged.
    pub display_name: Option<String>,
    /// The new review status; `None` leaves it unchanged.
    pub review_status: Option<ReviewStatus>,
    /// The detection under review; outer `None` leaves it, inner `None` clears it.
    pub detection_id: Option<Option<Uuid>>,
    /// The approved redaction; outer `None` leaves it, inner `None` clears it.
    pub redaction_id: Option<Option<Uuid>>,
}
