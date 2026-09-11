//! Workspace comments repository for threaded discussion on files.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceComment, UpdateWorkspaceComment, WorkspaceComment};
use crate::types::{AccountRefRow, CommentFilter, CursorPage, CursorPagination, WithAccountRef};
use crate::{Error, PgConnection, Result, schema};

/// The result of a [`create_comment`](WorkspaceCommentRepository::create_comment)
/// call whose parent reference is invalid.
///
/// Returned instead of a raw FK error so the handler can map a bad reply target
/// to a clear client error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyParentError {
    /// The named parent does not exist in this workspace (or is deleted).
    NotFound,
    /// The named parent is on a different file than the reply.
    FileMismatch,
    /// The named parent is itself a reply; threads are only one level deep.
    NotTopLevel,
}

/// Repository for workspace comment database operations.
///
/// Comments are threaded one level deep (a top-level comment and its direct
/// replies) and scoped to a file. Resolution and soft-delete are their own
/// audited operations.
pub trait WorkspaceCommentRepository {
    /// Creates a top-level comment. Does not validate a parent (see
    /// [`create_reply`](Self::create_reply) for replies).
    fn create_comment(
        &mut self,
        new_comment: NewWorkspaceComment,
    ) -> impl Future<Output = Result<WorkspaceComment>> + Send;

    /// Creates a reply to `parent_id`, validating that the parent exists in the
    /// same workspace and file and is itself top-level. Returns the offending
    /// [`ReplyParentError`] otherwise.
    fn create_reply(
        &mut self,
        new_comment: NewWorkspaceComment,
    ) -> impl Future<Output = Result<std::result::Result<WorkspaceComment, ReplyParentError>>> + Send;

    /// Finds a live comment by id within a specific workspace.
    fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceComment>>> + Send;

    /// Lists a file's live comments, oldest first (a thread reads top to bottom),
    /// each paired with the author's account reference.
    fn list_file_comments(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceComment>>>> + Send;

    /// Lists a workspace's live comments with cursor pagination, each paired with
    /// the author's account reference.
    fn cursor_list_workspace_comments(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &CommentFilter,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceComment>>>> + Send;

    /// Updates a comment's body.
    fn update_comment_body(
        &mut self,
        comment_id: Uuid,
        updates: UpdateWorkspaceComment,
    ) -> impl Future<Output = Result<WorkspaceComment>> + Send;

    /// Resolves a comment thread, recording who resolved it. A no-op timestamp
    /// change if already resolved.
    fn resolve_comment(
        &mut self,
        comment_id: Uuid,
        resolved_by: Uuid,
    ) -> impl Future<Output = Result<WorkspaceComment>> + Send;

    /// Reopens a resolved comment thread (clears the resolution).
    fn reopen_comment(
        &mut self,
        comment_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceComment>> + Send;

    /// Soft-deletes a comment.
    fn delete_comment(&mut self, comment_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceCommentRepository for PgConnection {
    async fn create_comment(
        &mut self,
        new_comment: NewWorkspaceComment,
    ) -> Result<WorkspaceComment> {
        use schema::workspace_comments;

        diesel::insert_into(workspace_comments::table)
            .values(&new_comment)
            .returning(WorkspaceComment::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn create_reply(
        &mut self,
        new_comment: NewWorkspaceComment,
    ) -> Result<std::result::Result<WorkspaceComment, ReplyParentError>> {
        let Some(parent_id) = new_comment.parent_id else {
            // A reply must name a parent; a missing one is a caller contract
            // violation, not a client-facing reply error.
            return Err(Error::unexpected("create_reply called without a parent_id"));
        };

        let parent = self
            .find_comment_in_workspace(new_comment.workspace_id, parent_id)
            .await?;
        let Some(parent) = parent else {
            return Ok(Err(ReplyParentError::NotFound));
        };
        if parent.file_id != new_comment.file_id {
            return Ok(Err(ReplyParentError::FileMismatch));
        }
        if parent.parent_id.is_some() {
            return Ok(Err(ReplyParentError::NotTopLevel));
        }

        let comment = self.create_comment(new_comment).await?;
        Ok(Ok(comment))
    }

    async fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> Result<Option<WorkspaceComment>> {
        use schema::workspace_comments::{self, dsl};

        workspace_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceComment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_file_comments(
        &mut self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<WithAccountRef<WorkspaceComment>>> {
        use schema::workspace_comments::dsl;
        use schema::{accounts, workspace_comments};

        // The author is one of two account FKs on the row (the other is
        // resolved_by), so the join names the column explicitly.
        let rows: Vec<(WorkspaceComment, AccountRefRow)> = workspace_comments::table
            .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceComment::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            // Oldest first: a thread reads top to bottom.
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect())
    }

    async fn cursor_list_workspace_comments(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &CommentFilter,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceComment>>> {
        use schema::workspace_comments::dsl;
        use schema::{accounts, workspace_comments};

        // One scoped builder for both the count and the page, so a future filter
        // cannot be added to one and forgotten on the other. The author is one of
        // two account FKs, so the join names it explicitly.
        let scoped = || {
            let mut query = workspace_comments::table
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
            if let Some(resolved) = filter.resolved {
                query = if resolved {
                    query.filter(dsl::resolved_at.is_not_null())
                } else {
                    query.filter(dsl::resolved_at.is_null())
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
            WorkspaceComment::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let rows: Vec<(WorkspaceComment, AccountRefRow)> = if let Some(cursor) = &pagination.after {
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

        let items: Vec<WithAccountRef<WorkspaceComment>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.item.created_at.into(), row.item.id)
        }))
    }

    async fn update_comment_body(
        &mut self,
        comment_id: Uuid,
        updates: UpdateWorkspaceComment,
    ) -> Result<WorkspaceComment> {
        use schema::workspace_comments::{self, dsl};

        // Scope to a live row so an edit cannot revive a soft-deleted comment.
        diesel::update(
            workspace_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(WorkspaceComment::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn resolve_comment(
        &mut self,
        comment_id: Uuid,
        resolved_by: Uuid,
    ) -> Result<WorkspaceComment> {
        use schema::workspace_comments::{self, dsl};

        diesel::update(
            workspace_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((dsl::resolved_at.eq(now), dsl::resolved_by.eq(resolved_by)))
        .returning(WorkspaceComment::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn reopen_comment(&mut self, comment_id: Uuid) -> Result<WorkspaceComment> {
        use schema::workspace_comments::{self, dsl};

        diesel::update(
            workspace_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((
            dsl::resolved_at.eq(None::<jiff_diesel::Timestamp>),
            dsl::resolved_by.eq(None::<Uuid>),
        ))
        .returning(WorkspaceComment::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn delete_comment(&mut self, comment_id: Uuid) -> Result<()> {
        use schema::workspace_comments::{self, dsl};

        // Scope to a live row so a concurrent delete is not overwritten with a
        // fresh `deleted_at`.
        diesel::update(
            workspace_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplyParentError, WorkspaceCommentRepository};
    use crate::model::{NewAccount, NewWorkspaceComment, UpdateWorkspaceComment};
    use crate::query::AccountRepository;
    use crate::test_util::TestDatabase;
    use crate::types::{CommentFilter, CursorPagination};

    #[tokio::test]
    async fn create_list_and_soft_delete() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let comment = conn
            .create_comment(NewWorkspaceComment::test(workspace_id, file_id, author))
            .await?;
        assert!(comment.parent_id.is_none());
        assert!(comment.resolved_at.is_none());

        let rows = conn.list_file_comments(workspace_id, file_id).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item.id, comment.id);

        // Soft-delete drops it from the listing.
        conn.delete_comment(comment.id).await?;
        assert!(
            conn.find_comment_in_workspace(workspace_id, comment.id)
                .await?
                .is_none()
        );
        assert!(
            conn.list_file_comments(workspace_id, file_id)
                .await?
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn replies_are_validated_one_level() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let top = conn
            .create_comment(NewWorkspaceComment::test(workspace_id, file_id, author))
            .await?;

        // A valid reply to a top-level comment.
        let reply = conn
            .create_reply(NewWorkspaceComment {
                parent_id: Some(top.id),
                ..NewWorkspaceComment::test(workspace_id, file_id, author)
            })
            .await?
            .expect("reply should be accepted");
        assert_eq!(reply.parent_id, Some(top.id));

        // A reply to a reply is rejected: threads are one level deep.
        let nested = conn
            .create_reply(NewWorkspaceComment {
                parent_id: Some(reply.id),
                ..NewWorkspaceComment::test(workspace_id, file_id, author)
            })
            .await?;
        assert_eq!(nested, Err(ReplyParentError::NotTopLevel));

        // A reply naming an unknown parent is rejected.
        let orphan = conn
            .create_reply(NewWorkspaceComment {
                parent_id: Some(uuid::Uuid::now_v7()),
                ..NewWorkspaceComment::test(workspace_id, file_id, author)
            })
            .await?;
        assert_eq!(orphan, Err(ReplyParentError::NotFound));
        Ok(())
    }

    #[tokio::test]
    async fn resolve_reopen_and_body_edit() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (author, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        let comment = conn
            .create_comment(NewWorkspaceComment::test(workspace_id, file_id, author))
            .await?;

        let resolved = conn.resolve_comment(comment.id, author).await?;
        assert!(resolved.resolved_at.is_some());
        assert_eq!(resolved.resolved_by, Some(author));

        let reopened = conn.reopen_comment(comment.id).await?;
        assert!(reopened.resolved_at.is_none());
        assert!(reopened.resolved_by.is_none());

        let edited = conn
            .update_comment_body(
                comment.id,
                UpdateWorkspaceComment {
                    body: Some("Edited body.".to_owned()),
                },
            )
            .await?;
        assert_eq!(edited.body, "Edited body.");
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_author_and_resolved() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (alice, workspace_id, _pipeline_id, file_id) = db.seed_pipeline_and_file().await;
        let bob = conn_seed_account(&db).await;
        let mut conn = db.client.get_connection().await?;

        let a = conn
            .create_comment(NewWorkspaceComment::test(workspace_id, file_id, alice))
            .await?;
        let _b = conn
            .create_comment(NewWorkspaceComment::test(workspace_id, file_id, bob))
            .await?;
        conn.resolve_comment(a.id, alice).await?;

        let all = conn
            .cursor_list_workspace_comments(
                workspace_id,
                CursorPagination::new(50),
                &CommentFilter::default(),
            )
            .await?;
        assert_eq!(all.items.len(), 2);

        let just_alice = conn
            .cursor_list_workspace_comments(
                workspace_id,
                CursorPagination::new(50),
                &CommentFilter {
                    author_account_id: Some(alice),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(just_alice.items.len(), 1);

        let resolved_only = conn
            .cursor_list_workspace_comments(
                workspace_id,
                CursorPagination::new(50),
                &CommentFilter {
                    resolved: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(resolved_only.items.len(), 1);
        assert_eq!(resolved_only.items[0].item.id, a.id);
        Ok(())
    }

    /// Seeds an extra account for the multi-author test.
    async fn conn_seed_account(db: &TestDatabase) -> uuid::Uuid {
        let mut conn = db.client.get_connection().await.expect("connection");
        conn.create_account(NewAccount::test())
            .await
            .expect("seed account")
            .id
    }
}
