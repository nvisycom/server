//! Workspace thread-comment repository: the messages within a thread. Includes
//! the reply path whose parent-scoped uniqueness makes a redelivered assistant
//! reply a no-op.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use super::workspace_thread_event::{StreamBound, TimelineCursor, TimelineSource};
use crate::model::{
    NewWorkspaceThreadComment, UpdateWorkspaceThreadComment, WorkspaceThreadComment,
};
use crate::types::{AccountRefRow, WithAccountRef};
use crate::{Error, PgConnection, Result, schema};

/// Read and write operations on a thread's comments.
pub trait WorkspaceThreadCommentRepository {
    /// Adds a comment (message) to a thread.
    fn create_comment(
        &mut self,
        new_comment: NewWorkspaceThreadComment,
    ) -> impl Future<Output = Result<WorkspaceThreadComment>> + Send;

    /// Adds a reply (a comment whose `parent_id` is set), returning `Ok(None)`
    /// when a live reply to that same parent already exists.
    ///
    /// The partial unique index on `parent_id` enforces at most one live reply per
    /// parent at the database level; the insert uses `ON CONFLICT DO NOTHING`
    /// against it, so a redelivered assistant job that re-inserts its reply yields
    /// no row (`Ok(None)`) rather than posting a duplicate or erroring.
    /// `new_comment.parent_id` must be set.
    fn create_reply(
        &mut self,
        new_comment: NewWorkspaceThreadComment,
    ) -> impl Future<Output = Result<Option<WorkspaceThreadComment>>> + Send;

    /// Finds a live comment by id within a workspace.
    fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceThreadComment>>> + Send;

    /// Lists a thread's live comments, oldest first, each paired with the author's
    /// account reference.
    fn list_thread_comments(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceThreadComment>>>> + Send;

    /// Lists up to `limit` of a thread's live comments at or after a cursor
    /// position, oldest first, each with the author's account reference. Backs the
    /// merged, paginated timeline; the caller interleaves these with the events.
    fn list_thread_comments_after(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceThreadComment>>>> + Send;

    /// Updates a comment's body.
    fn update_comment_body(
        &mut self,
        comment_id: Uuid,
        updates: UpdateWorkspaceThreadComment,
    ) -> impl Future<Output = Result<WorkspaceThreadComment>> + Send;

    /// Soft-deletes a comment.
    fn delete_comment(&mut self, comment_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceThreadCommentRepository for PgConnection {
    async fn create_comment(
        &mut self,
        new_comment: NewWorkspaceThreadComment,
    ) -> Result<WorkspaceThreadComment> {
        use schema::workspace_thread_comments;

        diesel::insert_into(workspace_thread_comments::table)
            .values(&new_comment)
            .returning(WorkspaceThreadComment::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn create_reply(
        &mut self,
        new_comment: NewWorkspaceThreadComment,
    ) -> Result<Option<WorkspaceThreadComment>> {
        use schema::workspace_thread_comments::{self, dsl};

        // `ON CONFLICT (parent_id) WHERE parent_id IS NOT NULL AND deleted_at IS
        // NULL DO NOTHING` targets the partial unique index: a live reply to this
        // parent already exists, so the insert returns no row and we report it as
        // "already replied" rather than posting a duplicate.
        diesel::insert_into(workspace_thread_comments::table)
            .values(&new_comment)
            .on_conflict(dsl::parent_id)
            .filter_target(dsl::parent_id.is_not_null().and(dsl::deleted_at.is_null()))
            .do_nothing()
            .returning(WorkspaceThreadComment::as_returning())
            .get_result(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> Result<Option<WorkspaceThreadComment>> {
        use schema::workspace_thread_comments::{self, dsl};

        workspace_thread_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceThreadComment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_thread_comments(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
    ) -> Result<Vec<WithAccountRef<WorkspaceThreadComment>>> {
        use schema::workspace_thread_comments::dsl;
        use schema::{accounts, workspace_thread_comments};

        let rows: Vec<(WorkspaceThreadComment, AccountRefRow)> = workspace_thread_comments::table
            .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::thread_id.eq(thread_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceThreadComment::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            // Oldest first: a discussion reads top to bottom.
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect())
    }

    async fn list_thread_comments_after(
        &mut self,
        workspace_id: Uuid,
        thread_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> Result<Vec<WithAccountRef<WorkspaceThreadComment>>> {
        use schema::workspace_thread_comments::dsl;
        use schema::{accounts, workspace_thread_comments};

        let mut query = workspace_thread_comments::table
            .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::thread_id.eq(thread_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply the per-stream keyset lower bound for this (comment) stream.
        if let Some(cursor) = after {
            match cursor.stream_bound(TimelineSource::Comment) {
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

        let rows: Vec<(WorkspaceThreadComment, AccountRefRow)> = query
            .select((
                WorkspaceThreadComment::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .limit(limit)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect())
    }

    async fn update_comment_body(
        &mut self,
        comment_id: Uuid,
        updates: UpdateWorkspaceThreadComment,
    ) -> Result<WorkspaceThreadComment> {
        use schema::workspace_thread_comments::{self, dsl};

        // An all-`None` changeset (here, no `body`) would make Diesel emit an empty
        // `SET` clause and fail with a query-builder error, so treat it as a no-op
        // and return the current comment unchanged.
        if updates.body.is_none() {
            return workspace_thread_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null())
                .select(WorkspaceThreadComment::as_select())
                .get_result(self)
                .await
                .map_err(Error::from);
        }

        diesel::update(
            workspace_thread_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(WorkspaceThreadComment::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn delete_comment(&mut self, comment_id: Uuid) -> Result<()> {
        use schema::workspace_thread_comments::{self, dsl};

        diesel::update(
            workspace_thread_comments::table
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
