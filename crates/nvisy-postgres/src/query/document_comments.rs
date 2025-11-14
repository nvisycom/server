//! Document comments repository for managing comment operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
use crate::{PgError, PgResult, schema};

/// Repository for document comment table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentCommentRepository;

impl DocumentCommentRepository {
    /// Creates a new document comment repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new comment in the database.
    pub async fn create_comment(
        conn: &mut AsyncPgConnection,
        new_comment: NewDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments;

        diesel::insert_into(document_comments::table)
            .values(&new_comment)
            .returning(DocumentComment::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a comment by its ID.
    pub async fn find_comment_by_id(
        conn: &mut AsyncPgConnection,
        comment_id: Uuid,
    ) -> PgResult<Option<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentComment::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds all comments for a document (including deleted ones).
    pub async fn find_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all comments for a document file.
    pub async fn find_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all comments for a document version.
    pub async fn find_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all replies to a comment (threaded replies).
    pub async fn find_comment_replies(
        conn: &mut AsyncPgConnection,
        parent_comment_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all top-level comments for a document.
    pub async fn find_top_level_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all top-level comments for a document file.
    pub async fn find_top_level_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all top-level comments for a document version.
    pub async fn find_top_level_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all comments by a specific account.
    pub async fn find_comments_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds all comments where a specific account was mentioned.
    pub async fn find_comments_mentioning_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::reply_to_account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates a comment by ID.
    pub async fn update_comment(
        conn: &mut AsyncPgConnection,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(&updates)
            .returning(DocumentComment::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes a comment by setting deleted_at timestamp.
    pub async fn delete_comment(conn: &mut AsyncPgConnection, comment_id: Uuid) -> PgResult<()> {
        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Counts total comments for a document.
    pub async fn count_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Counts total comments for a document file.
    pub async fn count_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Counts total comments for a document version.
    pub async fn count_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Counts replies for a specific comment.
    pub async fn count_comment_replies(
        conn: &mut AsyncPgConnection,
        parent_comment_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds recently created comments across all documents.
    pub async fn find_recent_comments(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        document_comments::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Checks if an account owns a specific comment.
    pub async fn check_comment_ownership(
        conn: &mut AsyncPgConnection,
        comment_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::document_comments::{self, dsl};

        let count: i64 = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}

/// Statistics for comments on a specific target.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentStats {
    /// Total number of comments
    pub total_count: i64,
    /// Number of top-level comments
    pub top_level_count: i64,
    /// Number of reply comments
    pub reply_count: i64,
}

impl CommentStats {
    /// Returns the average depth of comment threads.
    pub fn average_thread_depth(&self) -> f64 {
        if self.top_level_count == 0 {
            0.0
        } else {
            self.reply_count as f64 / self.top_level_count as f64
        }
    }

    /// Returns whether there are active discussions.
    pub fn has_active_discussions(&self) -> bool {
        self.total_count > 0
    }
}
