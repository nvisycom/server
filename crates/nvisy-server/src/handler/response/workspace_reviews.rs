//! Document-review response types: a review (its status, purpose, assignee, and
//! the id of the discussion thread it owns) and its activity-timeline events.

use jiff::Timestamp;
use nvisy_postgres::model::{
    WorkspaceReview as ReviewModel, WorkspaceReviewEvent as ReviewEventModel,
};
use nvisy_postgres::types::{ReviewEventKind, ReviewStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a document's review.
///
/// A review is an optional, purpose-scoped sign-off effort on a document (0..N per
/// document), opened explicitly. It owns a discussion thread (referenced by
/// `threadId`) and carries a `reviewStatus`, an optional `purpose`, and an optional
/// `assignee`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReview {
    /// Unique identifier of the review.
    pub id: Uuid,
    /// Document under review.
    pub document_id: Uuid,
    /// Discussion thread this review owns.
    pub thread_id: Uuid,
    /// Optional purpose/audience label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// The review's current status.
    pub review_status: ReviewStatus,
    /// Account the review is assigned to; `None` when unassigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<AccountRef>,
    /// When the review was created.
    pub created_at: Timestamp,
    /// When the review was last updated.
    pub updated_at: Timestamp,
}

/// Paginated response for reviews (the review queue, or a document's reviews).
pub type WorkspaceReviewsPage = Page<WorkspaceReview>;

impl WorkspaceReview {
    /// Creates a review response from the database model and the resolved assignee
    /// reference (absent when unassigned).
    #[must_use]
    pub fn from_model(review: &ReviewModel, assignee: Option<AccountRef>) -> Self {
        Self {
            id: review.id,
            document_id: review.document_id,
            thread_id: review.thread_id,
            purpose: review.purpose.clone(),
            review_status: review.review_status,
            assignee,
            created_at: review.created_at.into(),
            updated_at: review.updated_at.into(),
        }
    }
}

/// One entry in a review's activity timeline (a link, assignment, verification,
/// or reopen).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewEvent {
    /// Unique identifier of the event.
    pub id: Uuid,
    /// What happened.
    pub kind: ReviewEventKind,
    /// Account that performed the action; `None` if that account was removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<AccountRef>,
    /// Event-specific detail (the linked id, the assignee); `None` when none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<serde_json::Value>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// Paginated response for a review's activity timeline.
pub type WorkspaceReviewTimelinePage = Page<WorkspaceReviewEvent>;

impl WorkspaceReviewEvent {
    /// Creates a review-event response from the database model and the resolved
    /// actor reference (absent if the actor's account was removed).
    #[must_use]
    pub fn from_model(event: ReviewEventModel, actor: Option<AccountRef>) -> Self {
        Self {
            id: event.id,
            kind: event.kind,
            actor,
            target: event.target,
            created_at: event.created_at.into(),
        }
    }
}
