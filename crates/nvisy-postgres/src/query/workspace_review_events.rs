//! Workspace review-event repository: the immutable timeline entries (the review
//! lifecycle and its sign-off workflow) that the reader interleaves with the
//! comments. Also houses the shared `record_event` helper the review repository
//! uses to record those events.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceReviewEvent, WorkspaceReview, WorkspaceReviewComment, WorkspaceReviewEvent,
};
use crate::query::WorkspaceReviewCommentRepository;
use crate::types::{AccountRefRow, CursorPage, CursorPagination, ReviewEventKind, WithAccountRef};
use crate::{PgConnection, Result, schema};

/// Which of the two timeline streams an entry came from. Its order is the
/// tiebreak between an event and a comment that share a `created_at`: an event
/// sorts before a comment at the same instant. This is what makes a review's
/// opening render in the natural order — the `Opened` event is written first.
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

/// Keyset for the merged review timeline: comments and events ordered together by
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

/// One entry in a review's timeline: a comment or a lifecycle event, each with
/// its author or actor.
#[derive(Debug, Clone)]
pub enum TimelineEntry {
    /// A comment, with its author.
    Comment(WithAccountRef<WorkspaceReviewComment>),
    /// A lifecycle event, with its actor (absent when the account was removed).
    Event(WithAccountRef<WorkspaceReviewEvent, Option<AccountRefRow>>),
}

impl TimelineEntry {
    /// The entry's position in the merged timeline: `(created_at, source, id)`.
    /// Events sort before comments at the same instant; `id` breaks a tie within
    /// one stream. This is the total order the timeline is paginated by.
    #[must_use]
    pub fn cursor(&self) -> TimelineCursor {
        match self {
            Self::Comment(c) => TimelineCursor {
                created_at: c.item.created_at.into(),
                source: TimelineSource::Comment,
                id: c.item.id,
            },
            Self::Event(e) => TimelineCursor {
                created_at: e.item.created_at.into(),
                source: TimelineSource::Event,
                id: e.item.id,
            },
        }
    }
}

/// Read operations on a review's timeline events.
pub trait WorkspaceReviewEventRepository {
    /// Lists a review's timeline — its comments and lifecycle events interleaved,
    /// oldest first — one page after the pagination's cursor.
    ///
    /// Each stream contributes a bounded window (`limit + 1` rows from its own
    /// keyset bound). The windows are merged in timeline order and cut to the
    /// first `limit + 1`, of which the page keeps `limit`; the extra row signals a
    /// further page.
    fn list_review_timeline(
        &mut self,
        review_id: Uuid,
        pagination: &CursorPagination<TimelineCursor>,
    ) -> impl Future<Output = Result<CursorPage<TimelineEntry>>> + Send;

    /// Lists up to `limit` of a review's timeline events at or after a cursor
    /// position, oldest first, each with its actor's account reference. Backs the
    /// merged, paginated timeline; the caller interleaves these with the comments.
    fn list_review_events_after(
        &mut self,
        review_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> impl Future<
        Output = Result<Vec<WithAccountRef<WorkspaceReviewEvent, Option<AccountRefRow>>>>,
    > + Send;
}

impl WorkspaceReviewEventRepository for PgConnection {
    async fn list_review_timeline(
        &mut self,
        review_id: Uuid,
        pagination: &CursorPagination<TimelineCursor>,
    ) -> Result<CursorPage<TimelineEntry>> {
        let after = pagination.after_key();
        let fetch = pagination.fetch_limit();
        let comments = self
            .list_review_comments_after(review_id, after, fetch)
            .await?;
        let events = self
            .list_review_events_after(review_id, after, fetch)
            .await?;

        let mut entries: Vec<TimelineEntry> = comments
            .into_iter()
            .map(TimelineEntry::Comment)
            .chain(events.into_iter().map(TimelineEntry::Event))
            .collect();
        entries.sort_by_key(|entry| {
            let cursor = entry.cursor();
            (cursor.created_at, cursor.source, cursor.id)
        });
        // Each window holds up to `limit + 1` rows, so the merge can hold twice
        // that. Keep the first `limit + 1` in timeline order, so the page ends at
        // the true `limit`-th entry and the next cursor skips nothing.
        entries.truncate(usize::try_from(fetch).unwrap_or(usize::MAX));
        Ok(CursorPage::new(
            entries,
            None,
            pagination.limit,
            TimelineEntry::cursor,
        ))
    }

    async fn list_review_events_after(
        &mut self,
        review_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> Result<Vec<WithAccountRef<WorkspaceReviewEvent, Option<AccountRefRow>>>> {
        use schema::accounts;
        use schema::workspace_review_events::{self, dsl};

        let mut query = workspace_review_events::table
            .left_join(accounts::table.on(dsl::actor_account_id.eq(accounts::id.nullable())))
            .filter(dsl::review_id.eq(review_id))
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

        Ok(query
            .select((
                WorkspaceReviewEvent::as_select(),
                Option::<AccountRefRow>::as_select(),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .limit(limit)
            .load::<(WorkspaceReviewEvent, Option<AccountRefRow>)>(self)
            .await
            .map(|rows| rows.into_iter().map(WithAccountRef::from).collect())?)
    }
}

/// Inserts one review timeline event. Shared by the review repository, which
/// records events as part of its own transactions.
pub(crate) async fn record_event(
    conn: &mut PgConnection,
    review: &WorkspaceReview,
    kind: ReviewEventKind,
    actor: Uuid,
    target: Option<Value>,
) -> Result<()> {
    use schema::workspace_review_events;

    diesel::insert_into(workspace_review_events::table)
        .values(&NewWorkspaceReviewEvent {
            review_id: review.id,
            kind,
            actor_account_id: Some(actor),
            target,
        })
        .execute(conn)
        .await?;

    Ok(())
}
