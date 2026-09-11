//! Comment-thread response types.

use jiff::Timestamp;
use nvisy_postgres::model::{
    WorkspaceThread as ThreadModel, WorkspaceThreadAnchor as AnchorModel,
    WorkspaceThreadComment as CommentModel, WorkspaceThreadEvent as EventModel,
};
use nvisy_postgres::query::{TimelineCursor, TimelineSource};
use nvisy_postgres::types::ThreadEventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};
use crate::handler::request::CommentAnchor;

/// Response type for a comment thread.
///
/// A thread is the closable unit of discussion: opened by a workspace member,
/// optionally pinned to a file (`fileId`) and locations within it (`anchors`),
/// and closable to end the conversation. Its stream is a [`ThreadEntry`]
/// timeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    /// Unique identifier of the thread.
    pub id: Uuid,
    /// File the thread is pinned to; `None` for a workspace-level thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<Uuid>,
    /// The thread's title; `None` for an untitled thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Account that opened the thread.
    pub author: AccountRef,
    /// The thread's live anchors (locations within the file it is pinned to).
    /// Empty for a file-level or workspace-level thread.
    pub anchors: Vec<ThreadAnchor>,
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

/// Response type for a thread anchor: one location pin within the thread's file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadAnchor {
    /// Unique identifier of the anchor.
    pub id: Uuid,
    /// The modality-tagged location.
    pub anchor: CommentAnchor,
    /// When the anchor was added.
    pub created_at: Timestamp,
}

/// Response type for a comment: one message within a thread.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// Unique identifier of the comment.
    pub id: Uuid,
    /// Thread this message belongs to.
    pub thread_id: Uuid,
    /// Account that wrote the message.
    pub author: AccountRef,
    /// The message text.
    pub body: String,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
}

/// One non-message entry in a thread timeline (closed, reopened, anchor
/// added/removed).
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
    /// Event-specific detail (an anchor snapshot for anchor events); `None` for
    /// close/reopen.
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
    /// A lifecycle event (closed, reopened, anchor added/removed).
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
    /// Creates a thread response from the database model, its live anchors, and
    /// the resolved author reference.
    pub fn from_model(thread: ThreadModel, anchors: Vec<AnchorModel>, author: AccountRef) -> Self {
        Self {
            id: thread.id,
            file_id: thread.file_id,
            display_name: thread.display_name,
            author,
            anchors: anchors
                .into_iter()
                .filter_map(ThreadAnchor::from_model)
                .collect(),
            closed: thread.closed_at.is_some(),
            closed_at: thread.closed_at.map(Into::into),
            created_at: thread.created_at.into(),
            updated_at: thread.updated_at.into(),
        }
    }
}

impl ThreadAnchor {
    /// Creates an anchor response, decoding its stored JSON into the typed
    /// anchor. Returns `None` for an anchor whose JSON no longer decodes (treated
    /// as absent rather than failing the read).
    pub fn from_model(anchor: AnchorModel) -> Option<Self> {
        let decoded = serde_json::from_value(anchor.anchor).ok()?;
        Some(Self {
            id: anchor.id,
            anchor: decoded,
            created_at: anchor.created_at.into(),
        })
    }
}

impl Comment {
    /// Creates a comment response from the database model and the resolved author
    /// reference.
    pub fn from_model(comment: CommentModel, author: AccountRef) -> Self {
        Self {
            id: comment.id,
            thread_id: comment.thread_id,
            author,
            body: comment.body,
            created_at: comment.created_at.into(),
            updated_at: comment.updated_at.into(),
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
