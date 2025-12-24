//! Document comments repository for managing collaborative commenting operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for document comment database operations.
///
/// Handles comment lifecycle management including creation, threading, replies,
/// and mention tracking.
pub trait DocumentCommentRepository {
    fn create_comment(
        &self,
        new_comment: NewDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    fn find_comment_by_id(
        &self,
        comment_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentComment>>> + Send;

    fn find_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_comments_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_comments_mentioning_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn update_comment(
        &self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    fn delete_comment(&self, comment_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    fn count_comments_by_file(&self, file_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;

    fn check_comment_ownership(
        &self,
        comment_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl DocumentCommentRepository for PgClient {
    async fn create_comment(&self, new_comment: NewDocumentComment) -> PgResult<DocumentComment> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments;

        let comment = diesel::insert_into(document_comments::table)
            .values(&new_comment)
            .returning(DocumentComment::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comment)
    }

    /// Finds a comment by its unique identifier.
    ///
    /// Retrieves a specific comment using its UUID, automatically excluding
    /// soft-deleted comments. This is the primary method for accessing
    /// individual comments for viewing, editing, moderation, and threading
    /// operations within collaborative workflows.
    async fn find_comment_by_id(&self, comment_id: Uuid) -> PgResult<Option<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comment = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentComment::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(comment)
    }

    /// Finds all comments associated with a specific file.
    ///
    /// Retrieves comments targeted at a specific file, enabling file-specific
    /// discussions and focused feedback on particular content elements.
    async fn find_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Finds all comments created by a specific account.
    ///
    /// Retrieves a user's complete comment history, enabling user activity
    /// tracking, contribution analysis, and personal comment management.
    async fn find_comments_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Finds all comments where a specific account was mentioned or replied to.
    ///
    /// Retrieves comments that directly reference or respond to a specific
    /// account, enabling mention tracking and notification workflows.
    async fn find_comments_mentioning_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::reply_to_account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Updates a comment with new content and metadata.
    ///
    /// Applies partial updates to an existing comment using the provided
    /// update structure. Only fields set to `Some(value)` will be modified.
    async fn update_comment(
        &self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> PgResult<DocumentComment> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comment = diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(&updates)
            .returning(DocumentComment::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comment)
    }

    /// Soft deletes a comment by setting the deletion timestamp.
    ///
    /// Marks a comment as deleted without permanently removing it from the
    /// database. This preserves discussion context for audit purposes.
    async fn delete_comment(&self, comment_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Counts total active comments for a specific file.
    ///
    /// Calculates the total number of active comments associated with a
    /// file, providing discussion activity metrics and engagement indicators.
    async fn count_comments_by_file(&self, file_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Checks if an account owns a specific comment.
    ///
    /// Validates whether a user account is the original creator of a comment,
    /// supporting comment editing permissions and access control.
    async fn check_comment_ownership(&self, comment_id: Uuid, account_id: Uuid) -> PgResult<bool> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let count: i64 = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
