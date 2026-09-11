//! Workspace thread-event repository: the immutable timeline entries (opened,
//! closed, reopened, renamed, anchor added/removed) that the reader interleaves
//! with the comments. Also houses the shared helpers the thread and anchor
//! repositories use to record those events.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceThreadEvent, WorkspaceThread, WorkspaceThreadAnchor, WorkspaceThreadEvent,
};
use crate::types::{AccountRefRow, ThreadEventKind};
use crate::{Error, PgConnection, Result, schema};

/// Which of the two timeline streams an entry came from. Its order is the
/// tiebreak between an event and a comment that share a `created_at`: an event
/// sorts before a comment at the same instant. This is what makes a thread's
/// opening render in the natural order — the `Opened` event and the opening
/// comment are written in the same transaction (and can share an instant), and
/// the event is the one that logically comes first.
///
/// The variant order is significant: it is the derived `Ord` the timeline sorts
/// by, so `Event` must be declared before `Comment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineSource {
    /// A lifecycle event.
    Event,
    /// A comment (message).
    Comment,
}

/// Keyset for the merged thread timeline: comments and events ordered together by
/// `(created_at, source, id)` ascending. `source` breaks a `created_at` tie
/// between the two streams; `id` breaks a tie within one stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCursor {
    /// When the entry was created.
    pub created_at: Timestamp,
    /// Which stream the entry came from.
    pub source: TimelineSource,
    /// Entry id (tiebreaker within a stream).
    pub id: Uuid,
}

impl TimelineCursor {
    /// The lower bound for one stream's keyset query, given this cursor position.
    ///
    /// A stream's own [`TimelineSource`] decides how the cursor instant is
    /// treated:
    /// - source > cursor.source: every row at the cursor instant comes after it,
    ///   so include the whole instant.
    /// - source == cursor.source: only rows at the instant with a larger id.
    /// - source < cursor.source: no row at the instant qualifies; page strictly
    ///   after the instant.
    pub(crate) fn stream_bound(&self, source: TimelineSource) -> StreamBound {
        use std::cmp::Ordering::{Equal, Greater, Less};
        match source.cmp(&self.source) {
            Greater => StreamBound::FromInstant {
                created_at: self.created_at,
            },
            Equal => StreamBound::AfterId {
                created_at: self.created_at,
                id: self.id,
            },
            Less => StreamBound::AfterInstant {
                created_at: self.created_at,
            },
        }
    }
}

/// A single stream's keyset lower bound, derived from a [`TimelineCursor`].
pub(crate) enum StreamBound {
    /// Include rows strictly after `created_at`.
    AfterInstant { created_at: Timestamp },
    /// Include rows after `created_at`, plus rows at it with `id > id`.
    AfterId { created_at: Timestamp, id: Uuid },
    /// Include rows at or after `created_at` (the whole instant qualifies).
    FromInstant { created_at: Timestamp },
}

/// Read operations on a thread's timeline events.
pub trait WorkspaceThreadEventRepository {
    /// Lists a thread's timeline events, oldest first, each paired with the
    /// actor's account reference when the actor still exists.
    fn list_thread_events(
        &mut self,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>>> + Send;

    /// Lists up to `limit` of a thread's timeline events at or after a cursor
    /// position, oldest first, each with its actor's account reference. Backs the
    /// merged, paginated timeline; the caller interleaves these with the comments.
    fn list_thread_events_after(
        &mut self,
        thread_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>>> + Send;
}

impl WorkspaceThreadEventRepository for PgConnection {
    async fn list_thread_events(
        &mut self,
        thread_id: Uuid,
    ) -> Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>> {
        use schema::accounts;
        use schema::workspace_thread_events::{self, dsl};

        // The actor is nullable (SET NULL on account removal), so left-join it and
        // load the account-ref column group as an `Option`.
        workspace_thread_events::table
            .left_join(accounts::table.on(dsl::actor_account_id.eq(accounts::id.nullable())))
            .filter(dsl::thread_id.eq(thread_id))
            .select((
                WorkspaceThreadEvent::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                )
                    .nullable(),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn list_thread_events_after(
        &mut self,
        thread_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> Result<Vec<(WorkspaceThreadEvent, Option<AccountRefRow>)>> {
        use schema::accounts;
        use schema::workspace_thread_events::{self, dsl};

        let mut query = workspace_thread_events::table
            .left_join(accounts::table.on(dsl::actor_account_id.eq(accounts::id.nullable())))
            .filter(dsl::thread_id.eq(thread_id))
            .into_boxed();

        // Apply the per-stream keyset lower bound for this (event) stream.
        if let Some(cursor) = after {
            match cursor.stream_bound(TimelineSource::Event) {
                StreamBound::AfterInstant { created_at } => {
                    query =
                        query.filter(dsl::created_at.gt(jiff_diesel::Timestamp::from(created_at)));
                }
                StreamBound::AfterId { created_at, id } => {
                    let at = jiff_diesel::Timestamp::from(created_at);
                    query = query.filter(
                        dsl::created_at
                            .gt(at)
                            .or(dsl::created_at.eq(at).and(dsl::id.gt(id))),
                    );
                }
                StreamBound::FromInstant { created_at } => {
                    query =
                        query.filter(dsl::created_at.ge(jiff_diesel::Timestamp::from(created_at)));
                }
            }
        }

        query
            .select((
                WorkspaceThreadEvent::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                )
                    .nullable(),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .limit(limit)
            .load(self)
            .await
            .map_err(Error::from)
    }
}

/// Inserts one thread timeline event. Shared by the thread and anchor
/// repositories, which record events as part of their own transactions.
pub(crate) async fn record_event(
    conn: &mut PgConnection,
    thread: &WorkspaceThread,
    kind: ThreadEventKind,
    actor: Uuid,
    target: Option<Value>,
) -> Result<()> {
    use schema::workspace_thread_events;

    diesel::insert_into(workspace_thread_events::table)
        .values(&NewWorkspaceThreadEvent {
            workspace_id: thread.workspace_id,
            thread_id: thread.id,
            kind,
            actor_account_id: Some(actor),
            target,
        })
        .execute(conn)
        .await
        .map_err(Error::from)?;

    Ok(())
}

/// A JSON snapshot of an anchor for a timeline event's `target`, so the timeline
/// renders a removed anchor without its (now soft-deleted) row.
pub(crate) fn anchor_snapshot(anchor: &WorkspaceThreadAnchor) -> Value {
    serde_json::json!({ "anchorId": anchor.id, "anchor": anchor.anchor })
}
