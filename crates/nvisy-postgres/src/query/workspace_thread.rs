//! Workspace thread repository: the closable, optionally file-anchored unit of
//! discussion. Opening a thread creates its first comment and records the
//! `thread.opened` timeline event; closing, reopening, and renaming each record
//! their own event. Deleting a thread hides it and its comments.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value;
use uuid::Uuid;

use super::workspace_thread_event::record_event;
use crate::model::{
    NewWorkspaceThread, NewWorkspaceThreadAnchor, NewWorkspaceThreadComment, WorkspaceThread,
    WorkspaceThreadComment,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, ThreadEventKind, ThreadFilter, WithAccountRef,
};
use crate::{AsyncConnection, Error, PgConnection, Result, schema};

/// Read and write operations on threads.
pub trait WorkspaceThreadRepository {
    /// Opens a thread with its first comment and any initial anchors, recording
    /// the `thread.opened` timeline event, in one transaction. Returns the created
    /// thread and its opening comment.
    fn open_thread(
        &mut self,
        new_thread: NewWorkspaceThread,
        opening_body: String,
        anchors: Vec<Value>,
    ) -> impl Future<Output = Result<(WorkspaceThread, WorkspaceThreadComment)>> + Send;

    /// Finds a live thread by id within a workspace.
    fn find_thread_in_workspace(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThread>>> + Send;

    /// Lists a workspace's live threads with cursor pagination, each paired with
    /// the opening author's account reference.
    fn cursor_list_threads(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &ThreadFilter,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceThread>>>> + Send;

    /// Closes a thread, recording who closed it and a `thread.closed` timeline
    /// event, in one transaction. The caller checks the open state first.
    fn close_thread(
        &mut self,
        thread_id: Uuid,
        actor: Uuid,
    ) -> impl Future<Output = Result<WorkspaceThread>> + Send;

    /// Reopens a closed thread, recording a `thread.reopened` timeline event, in
    /// one transaction. The caller checks the closed state first.
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

    /// Soft-deletes a thread and all of its comments (its anchors and events are
    /// left in place, hidden with the thread).
    fn delete_thread(&mut self, thread_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceThreadRepository for PgConnection {
    async fn open_thread(
        &mut self,
        new_thread: NewWorkspaceThread,
        opening_body: String,
        anchors: Vec<Value>,
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

            // Initial anchors are part of the opening act, so they are recorded
            // without their own timeline events (the thread's creation covers it).
            if !anchors.is_empty() {
                use schema::workspace_thread_anchors;
                let rows: Vec<NewWorkspaceThreadAnchor> = anchors
                    .into_iter()
                    .map(|anchor| NewWorkspaceThreadAnchor {
                        thread_id: thread.id,
                        anchor,
                    })
                    .collect();
                diesel::insert_into(workspace_thread_anchors::table)
                    .values(&rows)
                    .execute(conn)
                    .await
                    .map_err(Error::from)?;
            }

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

    async fn cursor_list_threads(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
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
            if let Some(file_id) = filter.file_id {
                query = query.filter(dsl::file_id.eq(file_id));
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

        let query = scoped();
        let limit = pagination.fetch_limit();
        let selection = (
            WorkspaceThread::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let rows: Vec<(WorkspaceThread, AccountRefRow)> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);
            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(selection)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        } else {
            query
                .select(selection)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        };

        let items: Vec<WithAccountRef<WorkspaceThread>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.item.created_at.into(), row.item.id)
        }))
    }

    async fn close_thread(&mut self, thread_id: Uuid, actor: Uuid) -> Result<WorkspaceThread> {
        self.transaction(async |conn| {
            use schema::workspace_threads::{self, dsl};

            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null()),
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

            let thread = diesel::update(
                workspace_threads::table
                    .filter(dsl::id.eq(thread_id))
                    .filter(dsl::deleted_at.is_null()),
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
    use crate::model::{
        NewAccount, NewWorkspaceThread, NewWorkspaceThreadAnchor, NewWorkspaceThreadComment,
        UpdateWorkspaceThreadComment,
    };
    use crate::query::{
        AccountRepository, WorkspaceThreadAnchorRepository, WorkspaceThreadCommentRepository,
        WorkspaceThreadEventRepository, WorkspaceThreadRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::types::{CursorPagination, ThreadEventKind, ThreadFilter};

    #[tokio::test]
    async fn open_thread_creates_thread_and_opening_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, opening) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, author),
                "Opening message.".to_owned(),
                Vec::new(),
            )
            .await?;
        assert_eq!(thread.file_id, Some(file_id));
        assert!(thread.closed_at.is_none());
        assert_eq!(opening.thread_id, thread.id);
        assert_eq!(opening.body, "Opening message.");

        // A reply message lists after the opening one, oldest first.
        let _reply = conn
            .create_comment(NewWorkspaceThreadComment::test(
                workspace_id,
                thread.id,
                author,
            ))
            .await?;
        let msgs = conn.list_thread_comments(workspace_id, thread.id).await?;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].item.id, opening.id);
        Ok(())
    }

    #[tokio::test]
    async fn workspace_level_thread_has_no_file() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id) = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread {
                    workspace_id,
                    file_id: None,
                    author_account_id: author,
                    display_name: None,
                },
                "A general workspace discussion.".to_owned(),
                Vec::new(),
            )
            .await?;
        assert_eq!(thread.file_id, None);
        Ok(())
    }

    #[tokio::test]
    async fn close_reopen_records_timeline_events() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, author),
                "Opening.".to_owned(),
                Vec::new(),
            )
            .await?;

        let closed = conn.close_thread(thread.id, author).await?;
        assert!(closed.closed_at.is_some());
        assert_eq!(closed.closed_by, Some(author));

        let reopened = conn.reopen_thread(thread.id, author).await?;
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
            conn.find_thread_in_workspace(workspace_id, thread.id)
                .await?
                .is_none()
        );
        assert!(
            conn.list_thread_comments(workspace_id, thread.id)
                .await?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn anchors_add_remove_and_record_events() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        // Open with one initial anchor (the initial anchor gets no event of its
        // own; opening the thread records the single `thread.opened` event).
        let (thread, _opening) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, author),
                "Opening.".to_owned(),
                vec![serde_json::json!({ "modality": "text", "span": [0, 5] })],
            )
            .await?;
        assert_eq!(conn.list_thread_anchors(thread.id).await?.len(), 1);
        let opening_kinds: Vec<_> = conn
            .list_thread_events(thread.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(opening_kinds, vec![ThreadEventKind::Opened]);

        // Add a second anchor -> one anchor.added event.
        let added = conn
            .add_thread_anchor(
                workspace_id,
                NewWorkspaceThreadAnchor {
                    thread_id: thread.id,
                    anchor: serde_json::json!({ "modality": "text", "span": [10, 20] }),
                },
                author,
            )
            .await?;
        assert_eq!(conn.list_thread_anchors(thread.id).await?.len(), 2);

        // Remove it -> anchor.removed event; live anchors back to one.
        conn.remove_thread_anchor(workspace_id, added.id, author)
            .await?;
        assert_eq!(conn.list_thread_anchors(thread.id).await?.len(), 1);
        assert!(
            conn.find_thread_anchor(thread.id, added.id)
                .await?
                .is_none()
        );

        let kinds: Vec<_> = conn
            .list_thread_events(thread.id)
            .await?
            .into_iter()
            .map(|(e, _)| e.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ThreadEventKind::Opened,
                ThreadEventKind::AnchorAdded,
                ThreadEventKind::AnchorRemoved
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_threads_filters_by_author_and_closed() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (alice, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let bob = {
            let mut conn = db.client.get_connection().await?;
            conn.create_account(NewAccount::test()).await?.id
        };
        let mut conn = db.client.get_connection().await?;

        let (a, _) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, alice),
                "a".to_owned(),
                Vec::new(),
            )
            .await?;
        let _ = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, bob),
                "b".to_owned(),
                Vec::new(),
            )
            .await?;
        conn.close_thread(a.id, alice).await?;

        let all = conn
            .cursor_list_threads(
                workspace_id,
                CursorPagination::new(50),
                &ThreadFilter::default(),
            )
            .await?;
        assert_eq!(all.items.len(), 2);

        let closed_only = conn
            .cursor_list_threads(
                workspace_id,
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
                workspace_id,
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
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, opening) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, author),
                "Opening.".to_owned(),
                Vec::new(),
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
            conn.find_comment_in_workspace(workspace_id, opening.id)
                .await?
                .is_none()
        );
        // The thread still exists after deleting a message.
        assert!(
            conn.find_thread_in_workspace(workspace_id, thread.id)
                .await?
                .is_some()
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_reply_is_unique_per_triggering_comment() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let (thread, trigger) = conn
            .open_thread(
                NewWorkspaceThread::test(workspace_id, file_id, author),
                "@assistant help".to_owned(),
                Vec::new(),
            )
            .await?;

        let reply = |body: &str| NewWorkspaceThreadComment {
            workspace_id,
            thread_id: thread.id,
            author_account_id: author,
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
        let replies = conn.list_thread_comments(workspace_id, thread.id).await?;
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
}
