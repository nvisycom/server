//! Document comments repository for managing collaborative commenting operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use time::OffsetDateTime;
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

    fn find_comments_by_document(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_comment_replies(
        &self,
        parent_comment_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_top_level_comments_by_document(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

    fn find_top_level_comments_by_file(
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

    fn count_comments_by_document(
        &self,
        document_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn count_comments_by_file(&self, file_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;

    fn count_comment_replies(
        &self,
        parent_comment_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn find_recent_comments(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<DocumentComment>>> + Send;

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
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `comment_id` - UUID of the comment to retrieve
    ///
    /// # Returns
    ///
    /// The matching `DocumentComment` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
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

    /// Finds all comments associated with a specific document.
    ///
    /// Retrieves a comprehensive list of comments for a document including
    /// both active and deleted comments for complete discussion history.
    /// This supports full comment thread reconstruction and administrative
    /// moderation workflows where complete visibility is required.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document whose comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentComment` entries for the document, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_comments_by_document(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Finds all comments associated with a specific document file.
    ///
    /// Retrieves comments targeted at a specific file within a document,
    /// enabling file-specific discussions and focused feedback on particular
    /// content elements. This supports granular collaboration workflows
    /// where different files may have distinct discussion threads.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `file_id` - UUID of the document file whose comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of active `DocumentComment` entries for the file, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
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

    /// Finds all replies to a specific comment for threaded discussions.
    ///
    /// Retrieves child comments that directly reply to a parent comment,
    /// enabling hierarchical discussion threading and conversational flow.
    /// Results are ordered chronologically to preserve discussion sequence
    /// and maintain natural conversation threading.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `parent_comment_id` - UUID of the parent comment whose replies to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of reply `DocumentComment` entries ordered by creation time
    /// (oldest first to maintain conversation flow), or a database error if the query fails.
    async fn find_comment_replies(
        &self,
        parent_comment_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let replies = document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(replies)
    }

    /// Finds top-level comments for a document excluding replies.
    ///
    /// Retrieves only parent-level comments for a document, excluding threaded
    /// replies to present a clean top-level discussion overview. This supports
    /// discussion interfaces that display main conversation threads separately
    /// from their nested replies.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document whose top-level comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of top-level `DocumentComment` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
    async fn find_top_level_comments_by_document(
        &self,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::parent_comment_id.is_null())
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

    /// Finds top-level comments for a document file excluding replies.
    ///
    /// Retrieves only parent-level comments for a specific file within a document,
    /// excluding threaded replies to present a clean file-specific discussion
    /// overview. This enables focused discussions on individual content elements
    /// within larger document structures.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `file_id` - UUID of the document file whose top-level comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of top-level `DocumentComment` entries for the file ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_top_level_comments_by_file(
        &self,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::parent_comment_id.is_null())
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
    /// Retrieves a user's complete comment history across all documents,
    /// enabling user activity tracking, contribution analysis, and
    /// personal comment management workflows. This supports user
    /// dashboards and administrative oversight capabilities.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account whose comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentComment` entries created by the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
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
    /// This supports attention management and ensures users stay informed
    /// about relevant discussions and direct responses.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account that was mentioned or replied to
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `DocumentComment` entries mentioning the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
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
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. This supports comment editing,
    /// moderation actions, and status updates within collaborative workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `comment_id` - UUID of the comment to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `DocumentComment` with new values and timestamp,
    /// or a database error if the operation fails.
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
    /// database. This preserves discussion context for audit purposes and
    /// compliance requirements while preventing the comment from appearing
    /// in normal discussion threads. Threaded replies may be preserved
    /// to maintain conversation flow integrity.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `comment_id` - UUID of the comment to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Comment immediately becomes invisible in active discussions
    /// - Discussion context and audit trail is preserved
    /// - Thread structure may be maintained for conversation flow
    /// - Supports content moderation and user self-editing capabilities
    async fn delete_comment(&self, comment_id: Uuid) -> PgResult<()> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Counts total active comments for a specific document.
    ///
    /// Calculates the total number of active comments associated with a
    /// document, providing discussion activity metrics and engagement
    /// indicators for content management and analytics purposes.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `document_id` - UUID of the document to count comments for
    ///
    /// # Returns
    ///
    /// The total count of active comments for the document,
    /// or a database error if the query fails.
    async fn count_comments_by_document(&self, document_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Counts total active comments for a specific document file.
    ///
    /// Calculates the total number of active comments associated with a
    /// document file, providing file-specific discussion activity metrics
    /// and enabling granular content engagement analysis.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `file_id` - UUID of the document file to count comments for
    ///
    /// # Returns
    ///
    /// The total count of active comments for the file,
    /// or a database error if the query fails.
    async fn count_comments_by_file(&self, file_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Counts total replies for a specific parent comment.
    ///
    /// Calculates the total number of active replies to a parent comment,
    /// providing thread depth metrics and discussion engagement indicators
    /// for threaded conversation analysis and interface optimization.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `parent_comment_id` - UUID of the parent comment to count replies for
    ///
    /// # Returns
    ///
    /// The total count of active replies to the parent comment,
    /// or a database error if the query fails.
    async fn count_comment_replies(&self, parent_comment_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Finds recently created comments across all documents.
    ///
    /// Retrieves comments created within the last seven days across the
    /// entire system, providing visibility into recent discussion activity
    /// and enabling activity monitoring, trend analysis, and community
    /// engagement assessment.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `DocumentComment` entries ordered by
    /// creation time (most recent first), or a database error if the query fails.
    async fn find_recent_comments(&self, pagination: Pagination) -> PgResult<Vec<DocumentComment>> {
        let mut conn = self.get_connection().await?;

        use schema::document_comments::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        let comments = document_comments::table
            .filter(dsl::created_at.gt(seven_days_ago))
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

    /// Checks if an account owns a specific comment.
    ///
    /// Validates whether a user account is the original creator of a comment,
    /// supporting comment editing permissions, moderation capabilities, and
    /// access control for comment management operations. This enables
    /// user-specific comment administration and self-editing workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `comment_id` - UUID of the comment to check ownership for
    /// * `account_id` - UUID of the account claiming ownership
    ///
    /// # Returns
    ///
    /// `true` if the account owns the comment, `false` otherwise,
    /// or a database error if the query fails.
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
