//! Thread response types: the thread itself, its lifecycle events, and its
//! interleaved timeline.

use jiff::Timestamp;
use nvisy_postgres::model::{WorkspaceThread as ThreadModel, WorkspaceThreadEvent as EventModel};
use nvisy_postgres::query::{TimelineCursor, TimelineSource};
use nvisy_postgres::types::{ReviewStatus, ThreadEventKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Comment, Page};

/// Response type for a thread.
///
/// A thread is either a free-form workspace discussion (no `documentId`, opened
/// and closed by members) or a document's review (`documentId` set, one live
/// thread per document, auto-created on the document's first detection). A
/// document thread carries a derived `reviewStatus` and an optional `assignee`; a
/// workspace thread carries neither. Its stream is a [`ThreadEntry`] timeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    /// Unique identifier of the thread.
    pub id: Uuid,
    /// Document this thread reviews; `None` for a workspace-level thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<Uuid>,
    /// The thread's title; `None` for an untitled thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Account that opened the thread.
    pub author: AccountRef,
    /// The document review's current status, derived from the review timeline;
    /// `None` for a workspace thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_status: Option<ReviewStatus>,
    /// Account the document review is assigned to; `None` for a workspace thread
    /// or an unassigned document review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<AccountRef>,
    /// Whether the thread is closed.
    pub closed: bool,
    /// When the thread was closed, when closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<Timestamp>,
    /// When the thread was created.
    pub created_at: Timestamp,
    /// When the thread was last updated.
    pub updated_at: Timestamp,
}

/// One non-message entry in a thread timeline (opened, closed, reopened,
/// renamed, or a review transition).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEvent {
    /// Unique identifier of the event.
    pub id: Uuid,
    /// What happened.
    pub kind: ThreadEventKind,
    /// Account that performed the action; `None` if that account was removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<AccountRef>,
    /// Event-specific detail; `None` for events that carry none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<serde_json::Value>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// One entry in a thread's timeline: either a message or a lifecycle event,
/// tagged so a client renders them interleaved in order.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadEntry {
    /// A message posted in the thread.
    Comment(Comment),
    /// A lifecycle event (opened, closed, reopened, renamed, or a review
    /// transition).
    Event(ThreadEvent),
}

impl ThreadEntry {
    /// The entry's position in the merged timeline: `(created_at, source, id)`.
    /// Comments sort before events at the same instant; `id` breaks a tie within
    /// one stream. This is the total order the timeline is paginated by.
    pub fn cursor(&self) -> TimelineCursor {
        match self {
            ThreadEntry::Comment(c) => TimelineCursor {
                created_at: c.created_at,
                source: TimelineSource::Comment,
                id: c.id,
            },
            ThreadEntry::Event(e) => TimelineCursor {
                created_at: e.created_at,
                source: TimelineSource::Event,
                id: e.id,
            },
        }
    }

    /// The sort tuple for merging the two streams, derived from [`Self::cursor`].
    pub fn sort_key(&self) -> (Timestamp, TimelineSource, uuid::Uuid) {
        let c = self.cursor();
        (c.created_at, c.source, c.id)
    }
}

/// Paginated response for threads.
pub type ThreadsPage = Page<Thread>;

/// Paginated response for a thread's timeline.
pub type TimelinePage = Page<ThreadEntry>;

impl Thread {
    /// Creates a thread response from the database model, the resolved author
    /// reference, and the resolved assignee reference (absent for a workspace
    /// thread or an unassigned document review).
    pub fn from_model(
        thread: ThreadModel,
        author: AccountRef,
        assignee: Option<AccountRef>,
    ) -> Self {
        Self {
            id: thread.id,
            document_id: thread.document_id,
            display_name: thread.display_name,
            author,
            review_status: thread.review_status,
            assignee,
            closed: thread.closed_at.is_some(),
            closed_at: thread.closed_at.map(Into::into),
            created_at: thread.created_at.into(),
            updated_at: thread.updated_at.into(),
        }
    }
}

impl ThreadEvent {
    /// Creates a thread-event response from the database model and the resolved
    /// actor reference (absent if the actor's account was removed).
    pub fn from_model(event: EventModel, actor: Option<AccountRef>) -> Self {
        Self {
            id: event.id,
            kind: event.kind,
            actor,
            target: event.target,
            created_at: event.created_at.into(),
        }
    }
}
