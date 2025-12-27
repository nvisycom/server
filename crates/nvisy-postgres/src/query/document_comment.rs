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
    /// Creates a new document comment.
    fn create_comment(
        &self,
        new_comment: NewDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Finds a comment by its unique identifier.
    fn find_comment_by_id(
        &self,
        comment_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentComment>>> + Send;

    /// Finds all comments for a specific file.
    fn find_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Finds all comments created by a specific account.
    fn find_comments_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Finds comments where an account was mentioned or replied to.
    fn find_comments_mentioning_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Updates a comment with new content and metadata.
    fn update_comment(
        &self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Soft deletes a comment by setting the deletion timestamp.
    fn delete_comment(&self, comment_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Checks if an account owns a specific comment.
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
