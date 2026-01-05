//! Document comments repository for managing collaborative commenting operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for document comment database operations.
///
/// Handles comment lifecycle management including creation, threading, replies,
/// and mention tracking.
pub trait DocumentCommentRepository {
    /// Creates a new document comment.
    fn create_document_comment(
        &mut self,
        new_comment: NewDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Finds a document comment by its unique identifier.
    fn find_document_comment_by_id(
        &mut self,
        comment_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<DocumentComment>>> + Send;

    /// Lists document comments for a file with offset pagination.
    fn offset_list_file_document_comments(
        &mut self,
        file_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Lists document comments for a file with cursor pagination.
    fn cursor_list_file_document_comments(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<DocumentComment>>> + Send;

    /// Lists document comments created by an account with offset pagination.
    fn offset_list_account_document_comments(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Lists document comments created by an account with cursor pagination.
    fn cursor_list_account_document_comments(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<DocumentComment>>> + Send;

    /// Lists document comments mentioning an account with offset pagination.
    fn offset_list_document_comments_mentioning_account(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    /// Updates a document comment.
    fn update_document_comment(
        &mut self,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> impl Future<Output = PgResult<DocumentComment>> + Send;

    /// Soft deletes a document comment.
    fn delete_document_comment(
        &mut self,
        comment_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl DocumentCommentRepository for PgConnection {
    async fn create_document_comment(
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

    async fn find_document_comment_by_id(
        &mut self,
        comment_id: Uuid,
    ) -> PgResult<Option<DocumentComment>> {
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

    async fn offset_list_file_document_comments(
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

    async fn cursor_list_file_document_comments(
        &mut self,
        file_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<DocumentComment>> {
        use diesel::dsl::count_star;
        use schema::document_comments::{self, dsl};

        let base_filter = dsl::file_id.eq(file_id).and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                document_comments::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            document_comments::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentComment::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            document_comments::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentComment::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |c| {
            (c.created_at.into(), c.id)
        }))
    }

    async fn offset_list_account_document_comments(
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

    async fn cursor_list_account_document_comments(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<DocumentComment>> {
        use diesel::dsl::count_star;
        use schema::document_comments::{self, dsl};

        let base_filter = dsl::account_id
            .eq(account_id)
            .and(dsl::deleted_at.is_null());

        let total = if pagination.include_count {
            Some(
                document_comments::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            document_comments::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentComment::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            document_comments::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(DocumentComment::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |c| {
            (c.created_at.into(), c.id)
        }))
    }

    async fn offset_list_document_comments_mentioning_account(
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

    async fn update_document_comment(
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

    async fn delete_document_comment(&mut self, comment_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }
}
