//! Workspace review model: a named discussion on a document with a manual sign-off
//! lifecycle. A document has 0..N reviews. A review carries its [`ReviewStatus`],
//! owns a stream of [`WorkspaceReviewComment`] messages and [`WorkspaceReviewEvent`]
//! timeline entries, and references its assignees, detections, and redactions via
//! the link models ([`NewReviewAssignee`](super::NewReviewAssignee) et al.).
//!
//! [`WorkspaceReviewComment`]: super::WorkspaceReviewComment
//! [`WorkspaceReviewEvent`]: super::WorkspaceReviewEvent

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_reviews;
use crate::types::ReviewStatus;

/// A review: a named discussion on a document, carrying its [`ReviewStatus`]. Its
/// assignees, referenced detections, and referenced redactions live in link
/// tables; its comments and timeline events belong to it.
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
            author_account_id,
            display_name: "A test review.".to_owned(),
        }
    }
}

/// Data for updating a review's mutable fields.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_reviews)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceReview {
    /// The new title; `None` leaves it unchanged.
    pub display_name: Option<String>,
    /// The new review status; `None` leaves it unchanged.
    pub review_status: Option<ReviewStatus>,
}
