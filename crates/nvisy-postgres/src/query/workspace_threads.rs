//! Workspace thread repository. A thread is either a workspace discussion
//! (open/closed lifecycle) or a document's review (an assignee and a derived
//! [`ReviewStatus`], driven by review events). Opening a workspace thread creates
//! its first comment and records the `thread.opened` event; a document review thread
//! is created lazily by [`find_or_create_document_thread`] when the document is first
//! detected. Every lifecycle and review transition records its own timeline
//! event. Deleting a thread hides it and its comments.
//!
//! [`find_or_create_document_thread`]: WorkspaceThreadRepository::find_or_create_document_thread

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::workspace_thread_events::record_event;
use crate::model::{
    NewWorkspaceThread, NewWorkspaceThreadComment, WorkspaceThread, WorkspaceThreadComment,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, ReviewStatus, ThreadEventKind, ThreadFilter,
    WithAccountRef, keyset,
};
use crate::{AsyncConnection, Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's threads: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadCursor {
    /// When the thread was opened.
    pub created_at: Timestamp,
    /// Thread id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Read and write operations on threads.
pub trait WorkspaceThreadRepository {
    /// Opens a workspace discussion thread with its first comment, recording the
    /// `thread.opened` timeline event, in one transaction. Returns the created
    /// thread and its opening comment. For a document's review thread use
    /// [`find_or_create_document_thread`](Self::find_or_create_document_thread) instead.
    fn open_thread(
        &mut self,
        new_thread: NewWorkspaceThread,
        opening_body: String,
    ) -> impl Future<Output = Result<(WorkspaceThread, WorkspaceThreadComment)>> + Send;

    /// Returns the document's review thread, creating it if absent. A document has exactly
    /// one live review thread (the review of that document). On creation the thread
    /// starts at [`ReviewStatus::NeedsReview`] and records a
    /// `review.detection.created` event; it is idempotent, so a repeat detection
    /// returns the existing thread untouched.
    fn find_or_create_document_thread(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Finds a live thread by id within a workspace.
    fn find_thread_in_workspace(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThread>>> + Send;

    /// Finds the live review thread for a document, if one exists.
    fn find_document_thread(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThread>>> + Send;

    /// Finds a live thread by id within a workspace, taking a row lock (`FOR
    /// UPDATE`) so a concurrent close/reopen/delete serializes behind this read.
    ///
    /// Call inside a transaction that then acts on the thread's state (e.g.
    /// posting a comment only while it is open): the lock makes the check and the
    /// write atomic, closing the read-then-write race the unlocked
    /// [`find_thread_in_workspace`](Self::find_thread_in_workspace) leaves open.
    fn lock_thread_in_workspace(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThread>>> + Send;

    /// Lists a workspace's live threads with cursor pagination, each paired with
    /// the opening author's account reference.
    fn cursor_list_threads(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ThreadCursor>,
        filter: &ThreadFilter,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceThread>>>> + Send;

    /// Closes a workspace discussion thread, recording who closed it and a
    /// `thread.closed` timeline event, in one transaction. Applies only to
    /// workspace threads; a document review thread (which uses `review_status`) matches
    /// no row and returns `NotFound`. The caller checks the open state first.
    fn close_thread(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Reopens a closed workspace discussion thread, recording a `thread.reopened`
    /// timeline event, in one transaction. Applies only to workspace threads. The
    /// caller checks the closed state first.
    fn reopen_thread(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Sets a thread's title (or clears it with `None`), recording a
    /// `thread.renamed` timeline event carrying the new name, in one transaction.
    fn rename_thread(
        &mut self,
        thread_id: Uuid,
        display_name: Option<String>,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Marks a document review [`InReview`](ReviewStatus::InReview) (a redaction pass
    /// was made), recording a `review.redaction.created` event, unless it is
    /// already [`Resolved`](ReviewStatus::Resolved). Returns the thread.
    fn mark_review_in_review(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Verifies a document review, moving it to [`Resolved`](ReviewStatus::Resolved)
    /// and recording a `review.verified` event, in one transaction. Applies only
    /// to a document review that is not already resolved; anything else matches no row
    /// and returns `NotFound`.
    fn verify_review(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Reopens a resolved document review back to
    /// [`NeedsReview`](ReviewStatus::NeedsReview) (a new detection invalidated it),
    /// recording a `review.reopened` event. A no-op returning the thread when it is
    /// not resolved.
    fn reopen_review(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Sets or clears a document review's assignee, recording a `review.assigned` (or
    /// `review.unassigned` when clearing) event carrying the assignee, in one
    /// transaction.
    fn assign_review(
        &mut self,
        thread_id: Uuid,
        assignee: Option<Uuid>,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Soft-deletes a thread and all of its comments (its events are left in
    /// place, hidden with the thread).
    fn delete_thread(&mut self, thread_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceThreadRepository for PgConnection {
    async fn open_thread(
        &mut self,
        new_thread: NewWorkspaceThread,
        opening_body: String,
    ) -> Result<(WorkspaceThread, WorkspaceThreadComment)> {
        self.transaction(async |conn| {
            let thread = {
                use schema::workspace_threads;
                diesel::insert_into(workspace_threads::table)
                    .values(&new_thread)
                    .returning(WorkspaceThread::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?
            };

            // Record the thread's opening as the first timeline event, so the
            // stream begins with an explicit `thread.opened` entry.
            record_event(
                conn,
                &thread,
                ThreadEventKind::Opened,
                thread.author_account_id,
                None,
            )
            .await?;

            let opening = {
                use schema::workspace_thread_comments;
                diesel::insert_into(workspace_thread_comments::table)
                    .values(&NewWorkspaceThreadComment {
                        parent_id: None,
                        workspace_id: thread.workspace_id,
                        thread_id: thread.id,
                        author_account_id: thread.author_account_id,
                        body: opening_body,
                    })
                    .returning(WorkspaceThreadComment::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(Error::from)?
            };
            Ok((thread, opening))
        })
        .await
    }

    async fn find_or_create_document_thread(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // A document has exactly one live review thread. Lock it if it exists so a
            // concurrent detection does not create a second, then short-circuit.
            let existing = workspace_threads::table
                .filter(dsl::document_id.eq(document_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .select(WorkspaceThread::as_select())
                .for_update()
                .first(conn)
                .await
                .optional()
                .map_err(Error::from)?;
            if let Some(thread) = existing {
                return Ok(thread);
            }

            // First detection on this document: create its review thread at
            // `NeedsReview` and record the detection-created event. A concurrent
            // detection that raced past the lookup above hits the live-thread unique
            // index; `on_conflict_do_nothing` yields no row, and the existing thread
            // is re-read and returned rather than surfacing a unique violation.
            let created = diesel::insert_into(workspace_threads::table)
                .values(&NewWorkspaceThread {
                    workspace_id,
                    document_id: Some(document_id),
                    author_account_id: actor,
                    display_name: None,
                    review_status: Some(ReviewStatus::NeedsReview),
                })
                .on_conflict_do_nothing()
                .returning(WorkspaceThread::as_returning())
                .get_result(conn)
                .await
                .optional()
                .map_err(Error::from)?;

            let Some(thread) = created else {
                return workspace_threads::table
                    .filter(dsl::document_id.eq(document_id))
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .select(WorkspaceThread::as_select())
                    .first(conn)
                    .await
                    .map_err(Error::from);
            };

            record_event(
                conn,
                &thread,
                ThreadEventKind::DetectionCreated,
                actor,
                None,
            )
            .await?;

            Ok(thread)
        })
        .await
    }

    async fn find_thread_in_workspace(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> Result<Option<WorkspaceThread>> {
        use schema::workspace_threads::{self, dsl};

        workspace_threads::table
            .filter(dsl::id.eq(thread_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThread::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_document_thread(
        &mut self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<WorkspaceThread>> {
        use schema::workspace_threads::{self, dsl};

        workspace_threads::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThread::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn lock_thread_in_workspace(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> Result<Option<WorkspaceThread>> {
        use schema::workspace_threads::{self, dsl};

        workspace_threads::table
            .filter(dsl::id.eq(thread_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThread::as_select())
            .for_update()
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn cursor_list_threads(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ThreadCursor>,
        filter: &ThreadFilter,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceThread>>> {
        use schema::workspace_threads::dsl;
        use schema::{accounts, workspace_threads};

        // One scoped builder for both the count and the page. The opener is one of
        // two account FKs (the other is closed_by), so the join names it.
        let scoped = || {
            let mut query = workspace_threads::table
                .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed();
            if let Some(document_id) = filter.document_id {
                query = query.filter(dsl::document_id.eq(document_id));
            }
            if let Some(author_account_id) = filter.author_account_id {
                query = query.filter(dsl::author_account_id.eq(author_account_id));
            }
            if let Some(closed) = filter.closed {
                query = if closed {
                    query.filter(dsl::closed_at.is_not_null())
                } else {
                    query.filter(dsl::closed_at.is_null())
                };
            }
            if let Some(review_status) = filter.review_status {
                query = query.filter(dsl::review_status.eq(review_status));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let selection = (
            WorkspaceThread::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceThread, AccountRefRow)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select(selection)
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items: Vec<WithAccountRef<WorkspaceThread>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            ThreadCursor {
                created_at: row.item.created_at.into(),
                id: row.item.id,
            }
        }))
    }

    async fn close_thread(&mut self, thread_id: Uuid, actor: Uuid) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // The `document_id IS NULL` predicate limits this to workspace threads (a
            // document review uses `review_status`), and `closed_at IS NULL` makes the
            // transition atomic: an already-closed or document thread matches no row,
            // so the call returns `NotFound` rather than double-closing.
            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_null())
                    .filter(dsl::closed_at.is_null()),
            )
            .set((dsl::closed_at.eq(now), dsl::closed_by.eq(actor)))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            record_event(conn, &thread, ThreadEventKind::Closed, actor, None).await?;
            Ok(thread)
        })
        .await
    }

    async fn reopen_thread(&mut self, thread_id: Uuid, actor: Uuid) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // Workspace threads only (`document_id IS NULL`), and `closed_at IS NOT
            // NULL` makes it atomic: an already-open or document thread matches no row,
            // so a second concurrent reopen returns `NotFound` rather than
            // recording a duplicate `Reopened` event.
            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_null())
                    .filter(dsl::closed_at.is_not_null()),
            )
            .set((
                dsl::closed_at.eq(None::<jiff_diesel::Timestamp>),
                dsl::closed_by.eq(None::<Uuid>),
            ))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            record_event(conn, &thread, ThreadEventKind::Reopened, actor, None).await?;
            Ok(thread)
        })
        .await
    }

    async fn rename_thread(
        &mut self,
        thread_id: Uuid,
        display_name: Option<String>,
        actor: Uuid,
    ) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null()),
            )
            .set(dsl::display_name.eq(display_name.clone()))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            // The new name is the event's target so the timeline shows what it was
            // renamed to (a cleared name is recorded as JSON null).
            let target = serde_json::json!({ "displayName": display_name });
            record_event(conn, &thread, ThreadEventKind::Renamed, actor, Some(target)).await?;
            Ok(thread)
        })
        .await
    }

    async fn mark_review_in_review(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // A redaction on an already-resolved review does not undo the
            // verification: only a document review that is not yet resolved moves to
            // `in_review`. A redaction on a resolved review is a legitimate no-op
            // (the update matches no row), so fall back to reading the thread
            // rather than failing the redaction, and record no timeline event.
            let moved = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null())
                    .filter(dsl::review_status.ne(ReviewStatus::Resolved)),
            )
            .set(dsl::review_status.eq(ReviewStatus::InReview))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .optional()
            .map_err(Error::from)?;

            match moved {
                Some(thread) => {
                    record_event(
                        conn,
                        &thread,
                        ThreadEventKind::RedactionCreated,
                        actor,
                        None,
                    )
                    .await?;
                    Ok(thread)
                }
                None => workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null())
                    .select(WorkspaceThread::as_select())
                    .first(conn)
                    .await
                    .map_err(Error::from),
            }
        })
        .await
    }

    async fn verify_review(&mut self, thread_id: Uuid, actor: Uuid) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // Only a document review that is not already resolved can be verified; a
            // repeat verify (or a workspace thread) matches no row -> `NotFound`.
            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null())
                    .filter(dsl::review_status.ne(ReviewStatus::Resolved)),
            )
            .set(dsl::review_status.eq(ReviewStatus::Resolved))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            record_event(conn, &thread, ThreadEventKind::Verified, actor, None).await?;
            Ok(thread)
        })
        .await
    }

    async fn reopen_review(&mut self, thread_id: Uuid, actor: Uuid) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            // Only a resolved review reopens; a review still in progress is left as
            // is (the update matches no row, so fall back to reading the thread).
            let reopened = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null())
                    .filter(dsl::review_status.eq(ReviewStatus::Resolved)),
            )
            .set(dsl::review_status.eq(ReviewStatus::NeedsReview))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .optional()
            .map_err(Error::from)?;

            match reopened {
                Some(thread) => {
                    record_event(conn, &thread, ThreadEventKind::ReviewReopened, actor, None)
                        .await?;
                    Ok(thread)
                }
                None => workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null())
                    .select(WorkspaceThread::as_select())
                    .first(conn)
                    .await
                    .map_err(Error::from),
            }
        })
        .await
    }

    async fn assign_review(
        &mut self,
        thread_id: Uuid,
        assignee: Option<Uuid>,
        actor: Uuid,
    ) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null())
                    .filter(dsl::document_id.is_not_null()),
            )
            .set(dsl::assignee_account_id.eq(assignee))
            .returning(WorkspaceThread::as_returning())
            .get_result(conn)
            .await
            .map_err(Error::from)?;

            // The assignee is the event's target so the timeline shows who it went
            // to; clearing it records an unassigned event instead.
            let (kind, target) = match assignee {
                Some(id) => (
                    ThreadEventKind::Assigned,
                    serde_json::json!({ "assigneeAccountId": id }),
                ),
                None => (ThreadEventKind::Unassigned, serde_json::json!({})),
            };
            record_event(conn, &thread, kind, actor, Some(target)).await?;
            Ok(thread)
        })
        .await
    }

    async fn delete_thread(&mut self, thread_id: Uuid) -> Result<()> {
        self.transaction(async |conn| {
            use schema::{workspace_thread_comments, workspace_threads};

            // Soft-delete the thread and its live comments together, so a deleted
            // thread leaves no live messages behind. (The FK cascade only fires on
            // a hard delete; comments are hidden here by their own `deleted_at`.)
            diesel::update(
                workspace_threads::table
                    .filter(workspace_threads::id.eq(thread_id))
                    .filter(workspace_threads::deleted_at.is_null()),
            )
            .set(workspace_threads::deleted_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            diesel::update(
                workspace_thread_comments::table
                    .filter(workspace_thread_comments::thread_id.eq(thread_id))
                    .filter(workspace_thread_comments::deleted_at.is_null()),
            )
            .set(workspace_thread_comments::deleted_at.eq(now))
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::model::{
        NewAccount, NewWorkspaceThread, NewWorkspaceThreadComment, UpdateWorkspaceThreadComment,
    };
    use crate::query::{
        AccountRepository, TimelineCursor, TimelineSource, WorkspaceThreadCommentRepository,
        WorkspaceThreadEventRepository, WorkspaceThreadRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::types::{CursorPagination, ReviewStatus, ThreadEventKind, ThreadFilter};

    #[tokio::test]
    async fn open_thread_creates_thread_and_opening_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, opening) = conn
            .open_thread(
                NewWorkspaceThread::test(
                    seeded.workspace_id,
                    seeded.document_id,
                    seeded.account_id,
                ),
                "Opening message.".to_owned(),
            )
            .await?;
        assert_eq!(thread.document_id, Some(seeded.document_id));
        assert!(thread.closed_at.is_none());
        assert_eq!(opening.thread_id, thread.id);
        assert_eq!(opening.body, "Opening message.");

        // A reply message lists after the opening one, oldest first.
        let _reply = conn
            .create_comment(NewWorkspaceThreadComment::test(
                seeded.workspace_id,
                thread.id,
                seeded.account_id,
            ))
            .await?;
        let msgs = conn
            .list_thread_comments(seeded.workspace_id, thread.id)
            .await?;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].item.id, opening.id);
        Ok(())
    }

    #[tokio::test]
    async fn workspace_level_thread_has_no_file() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread {
                    workspace_id: seeded.workspace_id,
                    document_id: None,
                    author_account_id: seeded.account_id,
                    display_name: None,
                    review_status: None,
                },
                "A general workspace discussion.".to_owned(),
            )
            .await?;
        assert_eq!(thread.document_id, None);
        assert_eq!(thread.review_status, None);
        Ok(())
    }

    #[tokio::test]
    async fn close_reopen_records_timeline_events() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Close/reopen apply to workspace threads (no document), which use the
        // open/closed lifecycle rather than a review status.
        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread {
                    workspace_id: seeded.workspace_id,
                    document_id: None,
                    author_account_id: seeded.account_id,
                    display_name: None,
                    review_status: None,
                },
                "Opening.".to_owned(),
            )
            .await?;

        let closed = conn.close_thread(thread.id, seeded.account_id).await?;
        assert!(closed.closed_at.is_some());
        assert_eq!(closed.closed_by, Some(seeded.account_id));

        let reopened = conn.reopen_thread(thread.id, seeded.account_id).await?;
        assert!(reopened.closed_at.is_none());

        // The timeline records the open, then both transitions, oldest first.
        let events = conn.list_thread_events(thread.id).await?;
        let kinds: Vec<_> = events.iter().map(|(e, _)| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ThreadEventKind::Opened,
                ThreadEventKind::Closed,
                ThreadEventKind::Reopened
            ]
        );
        // The actor is resolved to an account ref (the opener/closer/reopener).
        assert!(events[0].1.is_some());

        // Deleting the thread hides it and its messages.
        conn.delete_thread(thread.id).await?;
        assert!(
            conn.find_thread_in_workspace(seeded.workspace_id, thread.id)
                .await?
                .is_none()
        );
        assert!(
            conn.list_thread_comments(seeded.workspace_id, thread.id)
                .await?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn file_thread_review_status_derives_from_events() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // A detection creates the document's review thread at NeedsReview and is
        // idempotent: a repeat detection returns the same thread.
        let thread = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await?;
        assert_eq!(thread.document_id, Some(seeded.document_id));
        assert_eq!(thread.review_status, Some(ReviewStatus::NeedsReview));
        let again = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await?;
        assert_eq!(again.id, thread.id);

        // A redaction moves it to InReview; verification to Resolved.
        let in_review = conn
            .mark_review_in_review(thread.id, seeded.account_id)
            .await?;
        assert_eq!(in_review.review_status, Some(ReviewStatus::InReview));
        let resolved = conn.verify_review(thread.id, seeded.account_id).await?;
        assert_eq!(resolved.review_status, Some(ReviewStatus::Resolved));

        // A re-detection after resolution reopens it to NeedsReview.
        let reopened = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await
            .map(|t| t.id)?;
        assert_eq!(reopened, thread.id);
        let reopened = conn.reopen_review(thread.id, seeded.account_id).await?;
        assert_eq!(reopened.review_status, Some(ReviewStatus::NeedsReview));

        // Exactly one live thread exists for the document throughout.
        assert!(
            conn.find_document_thread(seeded.workspace_id, seeded.document_id)
                .await?
                .is_some()
        );

        // The timeline records the review transitions in order.
        let kinds: Vec<_> = conn
            .list_thread_events(thread.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ThreadEventKind::DetectionCreated,
                ThreadEventKind::RedactionCreated,
                ThreadEventKind::Verified,
                ThreadEventKind::ReviewReopened,
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn assign_and_unassign_review_records_events() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let thread = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await?;
        assert_eq!(thread.assignee_account_id, None);

        let assigned = conn
            .assign_review(thread.id, Some(seeded.account_id), seeded.account_id)
            .await?;
        assert_eq!(assigned.assignee_account_id, Some(seeded.account_id));

        let unassigned = conn
            .assign_review(thread.id, None, seeded.account_id)
            .await?;
        assert_eq!(unassigned.assignee_account_id, None);

        let kinds: Vec<_> = conn
            .list_thread_events(thread.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ThreadEventKind::DetectionCreated,
                ThreadEventKind::Assigned,
                ThreadEventKind::Unassigned,
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_threads_filters_by_author_and_closed() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let new_workspace_thread = |author: Uuid| NewWorkspaceThread {
            workspace_id: seeded.workspace_id,
            document_id: None,
            author_account_id: author,
            display_name: None,
            review_status: None,
        };

        let (a, _) = conn
            .open_thread(new_workspace_thread(seeded.account_id), "a".to_owned())
            .await?;
        let _ = conn
            .open_thread(new_workspace_thread(bob), "b".to_owned())
            .await?;
        conn.close_thread(a.id, seeded.account_id).await?;

        let all = conn
            .cursor_list_threads(
                seeded.workspace_id,
                CursorPagination::new(50),
                &ThreadFilter::default(),
            )
            .await?;
        assert_eq!(all.items.len(), 2);

        let closed_only = conn
            .cursor_list_threads(
                seeded.workspace_id,
                CursorPagination::new(50),
                &ThreadFilter {
                    closed: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(closed_only.items.len(), 1);
        assert_eq!(closed_only.items[0].item.id, a.id);

        let just_bob = conn
            .cursor_list_threads(
                seeded.workspace_id,
                CursorPagination::new(50),
                &ThreadFilter {
                    author_account_id: Some(bob),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(just_bob.items.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn comment_edit_and_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, opening) = conn
            .open_thread(
                NewWorkspaceThread::test(
                    seeded.workspace_id,
                    seeded.document_id,
                    seeded.account_id,
                ),
                "Opening.".to_owned(),
            )
            .await?;

        let edited = conn
            .update_comment_body(
                opening.id,
                UpdateWorkspaceThreadComment {
                    body: Some("Edited.".to_owned()),
                },
            )
            .await?;
        assert_eq!(edited.body, "Edited.");

        conn.delete_comment(opening.id).await?;
        assert!(
            conn.find_comment_in_workspace(seeded.workspace_id, opening.id)
                .await?
                .is_none()
        );
        // The thread still exists after deleting a message.
        assert!(
            conn.find_thread_in_workspace(seeded.workspace_id, thread.id)
                .await?
                .is_some()
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_reply_is_unique_per_triggering_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, trigger) = conn
            .open_thread(
                NewWorkspaceThread::test(
                    seeded.workspace_id,
                    seeded.document_id,
                    seeded.account_id,
                ),
                "@assistant help".to_owned(),
            )
            .await?;

        let reply = |body: &str| NewWorkspaceThreadComment {
            workspace_id: seeded.workspace_id,
            thread_id: thread.id,
            author_account_id: seeded.account_id,
            parent_id: Some(trigger.id),
            body: body.to_owned(),
        };

        // The first reply to the triggering comment posts.
        let first = conn.create_reply(reply("first")).await?;
        assert!(first.is_some());

        // A second reply to the same comment is rejected by the partial unique
        // index and reported as "already replied" (None), never a duplicate.
        let second = conn.create_reply(reply("second")).await?;
        assert!(second.is_none());

        // Only the first reply is live.
        let replies = conn
            .list_thread_comments(seeded.workspace_id, thread.id)
            .await?;
        let bodies: Vec<_> = replies.iter().map(|r| r.item.body.as_str()).collect();
        assert!(bodies.contains(&"first"));
        assert!(!bodies.contains(&"second"));

        // After the first reply is soft-deleted, a new reply may be posted again
        // (the unique index is partial on live rows).
        conn.delete_comment(first.unwrap().id).await?;
        let third = conn.create_reply(reply("third")).await?;
        assert!(third.is_some());
        Ok(())
    }

    /// A merged-timeline page: one entry with its sort key, mirroring how the
    /// handler interleaves the two streams. `(created_at, source, id)`.
    type Entry = (jiff::Timestamp, TimelineSource, uuid::Uuid);

    /// Fetches one page of the merged timeline (comments + events) after `cursor`,
    /// mirroring the handler: pull `limit + 1` from each stream, merge by
    /// `(created_at, source, id)`, keep `limit`, and return the next cursor.
    async fn timeline_page(
        conn: &mut crate::PgConn,
        workspace_id: uuid::Uuid,
        thread_id: uuid::Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> anyhow::Result<(Vec<Entry>, Option<TimelineCursor>)> {
        let fetch = limit + 1;
        let comments = conn
            .list_thread_comments_after(workspace_id, thread_id, after, fetch)
            .await?;
        let events = conn
            .list_thread_events_after(thread_id, after, fetch)
            .await?;

        let mut merged: Vec<Entry> = Vec::new();
        merged.extend(
            comments
                .iter()
                .map(|c| (c.item.created_at.into(), TimelineSource::Comment, c.item.id)),
        );
        merged.extend(
            events
                .iter()
                .map(|(e, _)| (e.created_at.into(), TimelineSource::Event, e.id)),
        );
        merged.sort();

        let next = if merged.len() as i64 > limit {
            merged.truncate(limit as usize);
            merged
                .last()
                .map(|&(created_at, source, id)| TimelineCursor {
                    created_at,
                    source,
                    id,
                })
        } else {
            None
        };
        Ok((merged, next))
    }

    #[tokio::test]
    async fn timeline_pages_comments_and_events_in_one_order() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // Build a workspace thread with a known set of timeline entries: opening
        // (1 event + 1 comment), a reply comment, then close + reopen (2 events) =
        // 5 entries. A workspace thread is used because close/reopen apply to it.
        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread {
                    workspace_id: seeded.workspace_id,
                    document_id: None,
                    author_account_id: seeded.account_id,
                    display_name: None,
                    review_status: None,
                },
                "Opening.".to_owned(),
            )
            .await?;
        conn.create_comment(NewWorkspaceThreadComment::test(
            seeded.workspace_id,
            thread.id,
            seeded.account_id,
        ))
        .await?;
        conn.close_thread(thread.id, seeded.account_id).await?;
        conn.reopen_thread(thread.id, seeded.account_id).await?;

        // The full merged timeline (a big first page) is every entry in order.
        let (all, _) = timeline_page(&mut conn, seeded.workspace_id, thread.id, None, 50).await?;
        assert_eq!(all.len(), 5);
        // It is sorted ascending by (created_at, source, id).
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(all, sorted);

        // Paging in windows of 2 walks the same order with no gaps or repeats.
        let mut paged: Vec<Entry> = Vec::new();
        let mut cursor: Option<TimelineCursor> = None;
        loop {
            let (page, next) = timeline_page(
                &mut conn,
                seeded.workspace_id,
                thread.id,
                cursor.as_ref(),
                2,
            )
            .await?;
            paged.extend(page);
            match next {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(paged, all);
        Ok(())
    }

    #[tokio::test]
    async fn find_or_create_document_thread_is_idempotent() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_document().await;
        let mut conn = db.client.get_connection().await?;

        // The first call creates the document's review thread; a second call for the
        // same document returns that same thread rather than a second one or a
        // unique-violation error.
        let first = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await?;
        let second = conn
            .find_or_create_document_thread(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            )
            .await?;
        assert_eq!(second.id, first.id, "one live review thread per document");
        Ok(())
    }
}
