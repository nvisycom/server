//! Document comments repository for managing collaborative commenting operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
use crate::types::OffsetPagination;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document comment database operations.
///
/// Handles comment lifecycle management including creation, threading, replies,
/// and mention tracking.
pub trait DocumentCommentRepository {
    /// Creates a new document comment.
    fn create_comment(
        &mut self,
        new_comment: NewDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Finds a comment by its unique identifier.
    fn find_comment_by_id(
        &mut self,
        comment_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentComment>>> + Send;

    /// Lists all comments for a specific file with offset pagination.
    fn offset_list_file_comments(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Lists all comments created by a specific account with offset pagination.
    fn offset_list_account_comments(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Finds comments where an account was mentioned or replied to.
    fn find_comments_mentioning_account(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Updates a comment with new content and metadata.
    fn update_comment(
        &mut self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Soft deletes a comment by setting the deletion timestamp.
    fn delete_comment(&mut self, comment_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Checks if an account owns a specific comment.
    fn check_comment_ownership(
        &mut self,
        comment_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl DocumentCommentRepository for PgConnection {
    async fn create_comment(
        &mut self,
        new_comment: NewDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments;

        let comment = diesel::insert_into(document_comments::table)
            .values(&new_comment)
            .returning(DocumentComment::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(comment)
    }

    async fn find_comment_by_id(&mut self, comment_id: Uuid) -> PgResult<Option<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comment = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentComment::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(comment)
    }

    async fn offset_list_file_comments(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    async fn offset_list_account_comments(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    async fn find_comments_mentioning_account(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::reply_to_account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    async fn update_comment(
        &mut self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments::{self, dsl};

        let comment = diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(&updates)
            .returning(DocumentComment::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(comment)
    }

    async fn delete_comment(&mut self, comment_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn check_comment_ownership(
        &mut self,
        comment_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::document_comments::{self, dsl};

        let count: i64 = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
