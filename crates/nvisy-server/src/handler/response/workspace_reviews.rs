//! Review response types: the review itself, its comments, its timeline events,
//! and its interleaved timeline.

use jiff::Timestamp;
use nvisy_postgres::model::{
    WorkspaceReview as ReviewModel, WorkspaceReviewComment as CommentModel,
    WorkspaceReviewEvent as ReviewEventModel,
};
use nvisy_postgres::query::{TimelineCursor, TimelineSource};
use nvisy_postgres::types::{ReviewEventKind, ReviewStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a review: a named discussion on a document with a manual
/// sign-off lifecycle (0..N per document), opened explicitly. It carries a
/// `reviewStatus`, a `displayName`, and its `assignees` (0..N reviewers); its
/// stream is a [`WorkspaceReviewEntry`] timeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReview {
    /// Unique identifier of the review.
    pub id: Uuid,
    /// Document under review.
    pub document_id: Uuid,
    /// The review's title.
    pub display_name: String,
    /// Account that opened the review.
    pub author: AccountRef,
    /// The review's current status.
    pub review_status: ReviewStatus,
    /// The reviewers assigned to this review (empty when unassigned).
    pub assignees: Vec<AccountRef>,
    /// When the review was created.
    pub created_at: Timestamp,
    /// When the review was last updated.
    pub updated_at: Timestamp,
}

/// Paginated response for reviews (the review queue, or a document's reviews).
pub type WorkspaceReviewsPage = Page<WorkspaceReview>;

impl WorkspaceReview {
    /// Creates a review response from the database model, its author, and its
    /// resolved assignee references (empty when unassigned).
    #[must_use]
    pub fn from_model(
        review: &ReviewModel,
        author: AccountRef,
        assignees: Vec<AccountRef>,
    ) -> Self {
        Self {
            id: review.id,
            document_id: review.document_id,
            display_name: review.display_name.clone(),
            author,
            review_status: review.review_status,
            assignees,
            created_at: review.created_at.into(),
            updated_at: review.updated_at.into(),
        }
    }
}

/// Response type for a comment: one message within a review's discussion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceComment {
    /// Unique identifier of the comment.
    pub id: Uuid,
    /// Review this message belongs to.
    pub review_id: Uuid,
    /// Account that wrote the message.
    pub author: AccountRef,
    /// The message text.
    pub body: String,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
}

impl WorkspaceComment {
    /// Creates a comment response from the database model and the resolved author
    /// reference.
    #[must_use]
    pub fn from_model(comment: CommentModel, author: AccountRef) -> Self {
        Self {
            id: comment.id,
            review_id: comment.review_id,
            author,
            body: comment.body,
            created_at: comment.created_at.into(),
            updated_at: comment.updated_at.into(),
        }
    }
}

/// One non-message entry in a review timeline (opened, renamed, a detection or
/// redaction linked, an assignment change, verified, or reopened).
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
    /// Event-specific detail (the new name, a linked id, an assignee); `None` when
    /// none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<serde_json::Value>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// Paginated response for a review's activity timeline (its events only).
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

/// One entry in a review's timeline: either a message or a lifecycle event, tagged
/// so a client renders them interleaved in order.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceReviewEntry {
    /// A message posted in the review.
    Comment(WorkspaceComment),
    /// A lifecycle event.
    Event(WorkspaceReviewEvent),
}

impl WorkspaceReviewEntry {
    /// The entry's position in the merged timeline: `(created_at, source, id)`.
    /// Comments sort after events at the same instant; `id` breaks a tie within one
    /// stream. This is the total order the timeline is paginated by.
    #[must_use]
    pub fn cursor(&self) -> TimelineCursor {
        match self {
            WorkspaceReviewEntry::Comment(c) => TimelineCursor {
                created_at: c.created_at,
                source: TimelineSource::Comment,
                id: c.id,
            },
            WorkspaceReviewEntry::Event(e) => TimelineCursor {
                created_at: e.created_at,
                source: TimelineSource::Event,
                id: e.id,
            },
        }
    }

    /// The sort tuple for merging the two streams, derived from [`Self::cursor`].
    #[must_use]
    pub fn sort_key(&self) -> (Timestamp, TimelineSource, uuid::Uuid) {
        let c = self.cursor();
        (c.created_at, c.source, c.id)
    }
}

/// Paginated response for a review's interleaved timeline (comments + events).
pub type WorkspaceTimelinePage = Page<WorkspaceReviewEntry>;
