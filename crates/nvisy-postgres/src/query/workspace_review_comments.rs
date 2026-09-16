//! Workspace review-comment repository: the messages within a review's discussion.
//! Includes the reply path whose parent-scoped uniqueness makes a redelivered
//! assistant reply a no-op.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use super::workspace_review_events::{StreamBound, TimelineCursor, TimelineSource};
use crate::model::{
    NewWorkspaceReviewComment, UpdateWorkspaceReviewComment, WorkspaceReviewComment,
};
use crate::types::{AccountRefRow, WithAccountRef};
use crate::{Error, PgConnection, Result, schema};

/// Read and write operations on a review's comments.
pub trait WorkspaceReviewCommentRepository {
    /// Adds a comment (message) to a review.
    fn create_comment(
        &mut self,
        new_comment: NewWorkspaceReviewComment,
    ) -> impl Future<Output = Result<WorkspaceReviewComment>> + Send;

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
        new_comment: NewWorkspaceReviewComment,
    ) -> impl Future<Output = Result<Option<WorkspaceReviewComment>>> + Send;

    /// Finds a live comment by id within a workspace.
    fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceReviewComment>>> + Send;

    /// Lists a review's live comments, oldest first, each paired with the author's
    /// account reference.
    fn list_review_comments(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceReviewComment>>>> + Send;

    /// Lists up to `limit` of a review's live comments at or after a cursor
    /// position, oldest first, each with the author's account reference. Backs the
    /// merged, paginated timeline; the caller interleaves these with the events.
    fn list_review_comments_after(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceReviewComment>>>> + Send;

    /// Updates a comment's body.
    fn update_comment_body(
        &mut self,
        comment_id: Uuid,
        updates: UpdateWorkspaceReviewComment,
    ) -> impl Future<Output = Result<WorkspaceReviewComment>> + Send;

    /// Soft-deletes a comment.
    fn delete_comment(&mut self, comment_id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceReviewCommentRepository for PgConnection {
    async fn create_comment(
        &mut self,
        new_comment: NewWorkspaceReviewComment,
    ) -> Result<WorkspaceReviewComment> {
        use schema::workspace_review_comments;

        diesel::insert_into(workspace_review_comments::table)
            .values(&new_comment)
            .returning(WorkspaceReviewComment::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn create_reply(
        &mut self,
        new_comment: NewWorkspaceReviewComment,
    ) -> Result<Option<WorkspaceReviewComment>> {
        use schema::workspace_review_comments::{self, dsl};

        // `ON CONFLICT (parent_id) WHERE parent_id IS NOT NULL AND deleted_at IS
        // NULL DO NOTHING` targets the partial unique index: a live reply to this
        // parent already exists, so the insert returns no row and we report it as
        // "already replied" rather than posting a duplicate.
        diesel::insert_into(workspace_review_comments::table)
            .values(&new_comment)
            .on_conflict(dsl::parent_id)
            .filter_target(dsl::parent_id.is_not_null().and(dsl::deleted_at.is_null()))
            .do_nothing()
            .returning(WorkspaceReviewComment::as_returning())
            .get_result(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_comment_in_workspace(
        &mut self,
        workspace_id: Uuid,
        comment_id: Uuid,
    ) -> Result<Option<WorkspaceReviewComment>> {
        use schema::workspace_review_comments::{self, dsl};

        workspace_review_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceReviewComment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_review_comments(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
    ) -> Result<Vec<WithAccountRef<WorkspaceReviewComment>>> {
        use schema::workspace_review_comments::dsl;
        use schema::{accounts, workspace_review_comments};

        let rows: Vec<(WorkspaceReviewComment, AccountRefRow)> = workspace_review_comments::table
            .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::review_id.eq(review_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceReviewComment::as_select(),
                (
                    accounts::id,
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

    async fn list_review_comments_after(
        &mut self,
        workspace_id: Uuid,
        review_id: Uuid,
        after: Option<&TimelineCursor>,
        limit: i64,
    ) -> Result<Vec<WithAccountRef<WorkspaceReviewComment>>> {
        use schema::workspace_review_comments::dsl;
        use schema::{accounts, workspace_review_comments};

        let mut query = workspace_review_comments::table
            .inner_join(accounts::table.on(dsl::author_account_id.eq(accounts::id)))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::review_id.eq(review_id))
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

        let rows: Vec<(WorkspaceReviewComment, AccountRefRow)> = query
            .select((
                WorkspaceReviewComment::as_select(),
                (
                    accounts::id,
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
        updates: UpdateWorkspaceReviewComment,
    ) -> Result<WorkspaceReviewComment> {
        use schema::workspace_review_comments::{self, dsl};

        // An all-`None` changeset (here, no `body`) would make Diesel emit an empty
        // `SET` clause and fail with a query-builder error, so treat it as a no-op
        // and return the current comment unchanged.
        if updates.body.is_none() {
            return workspace_review_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null())
                .select(WorkspaceReviewComment::as_select())
                .get_result(self)
                .await
                .map_err(Error::from);
        }

        diesel::update(
            workspace_review_comments::table
                .filter(dsl::id.eq(comment_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(WorkspaceReviewComment::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn delete_comment(&mut self, comment_id: Uuid) -> Result<()> {
        use schema::workspace_review_comments::{self, dsl};

        diesel::update(
            workspace_review_comments::table
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
