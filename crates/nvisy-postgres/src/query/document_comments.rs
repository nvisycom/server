//! Document comments repository for managing collaborative commenting operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{DocumentComment, NewDocumentComment, UpdateDocumentComment};
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive document comment database operations.
///
/// Provides database operations for managing document comments throughout their
/// lifecycle, including creation, updates, threading management, and moderation
/// capabilities. This repository handles all database interactions related to
/// collaborative commenting on documents, files, and versions, serving as the
/// primary interface for discussion and feedback workflows.
///
/// The repository supports hierarchical comment threading, reply management,
/// mention tracking, and comprehensive search and filtering capabilities to
/// enable rich collaborative discussion experiences. Comments facilitate
/// knowledge sharing, peer review, and iterative content improvement within
/// document-centric workflows.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentCommentRepository;

impl DocumentCommentRepository {
    /// Creates a new document comment repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `DocumentCommentRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new comment in the database with complete threading support.
    ///
    /// Initializes a new comment within the collaborative discussion system
    /// with support for document, file, or version-specific commenting.
    /// The comment is immediately available for viewing and replying,
    /// enabling real-time collaborative feedback and discussion workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_comment` - Complete comment data including content and target references
    ///
    /// # Returns
    ///
    /// The created `DocumentComment` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Comment becomes immediately visible to project collaborators
    /// - Enables threaded discussions and knowledge sharing
    /// - Supports peer review and iterative content improvement
    /// - Facilitates asynchronous collaboration across time zones
    /// - Creates audit trail for content evolution and decision making
    pub async fn create_comment(
        conn: &mut AsyncPgConnection,
        new_comment: NewDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments;

        let comment = diesel::insert_into(document_comments::table)
            .values(&new_comment)
            .returning(DocumentComment::as_returning())
            .get_result(conn)
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
    pub async fn find_comment_by_id(
        conn: &mut AsyncPgConnection,
        comment_id: Uuid,
    ) -> PgResult<Option<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comment = document_comments::table
            .filter(dsl::id.eq(comment_id))
            .filter(dsl::deleted_at.is_null())
            .select(DocumentComment::as_select())
            .first(conn)
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
    pub async fn find_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Finds all comments associated with a specific document version.
    ///
    /// Retrieves comments targeted at a specific version of a document,
    /// enabling version-specific discussions and historical feedback tracking.
    /// This supports version-controlled collaboration workflows where
    /// feedback may be tied to particular content iterations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `version_id` - UUID of the document version whose comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of active `DocumentComment` entries for the version, ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn find_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_comment_replies(
        conn: &mut AsyncPgConnection,
        parent_comment_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let replies = document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_top_level_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_top_level_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(comments)
    }

    /// Finds top-level comments for a document version excluding replies.
    ///
    /// Retrieves only parent-level comments for a specific document version,
    /// excluding threaded replies to present a clean version-specific discussion
    /// overview. This supports version-controlled feedback workflows where
    /// discussions may be tied to particular content iterations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `version_id` - UUID of the document version whose top-level comments to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of top-level `DocumentComment` entries for the version ordered by
    /// creation time (most recent first), or a database error if the query fails.
    pub async fn find_top_level_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::parent_comment_id.is_null())
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_comments_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn find_comments_mentioning_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let comments = document_comments::table
            .filter(dsl::reply_to_account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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
    pub async fn update_comment(
        conn: &mut AsyncPgConnection,
        comment_id: Uuid,
        updates: UpdateDocumentComment,
    ) -> PgResult<DocumentComment> {
        use schema::document_comments::{self, dsl};

        let comment = diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(&updates)
            .returning(DocumentComment::as_returning())
            .get_result(conn)
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
    pub async fn delete_comment(conn: &mut AsyncPgConnection, comment_id: Uuid) -> PgResult<()> {
        use schema::document_comments::{self, dsl};

        diesel::update(document_comments::table.filter(dsl::id.eq(comment_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
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
    pub async fn count_comments_by_document(
        conn: &mut AsyncPgConnection,
        document_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::document_id.eq(document_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
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
    pub async fn count_comments_by_file(
        conn: &mut AsyncPgConnection,
        file_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::document_file_id.eq(file_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Counts total active comments for a specific document version.
    ///
    /// Calculates the total number of active comments associated with a
    /// document version, providing version-specific discussion activity
    /// metrics and supporting version-controlled feedback analysis.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `version_id` - UUID of the document version to count comments for
    ///
    /// # Returns
    ///
    /// The total count of active comments for the version,
    /// or a database error if the query fails.
    pub async fn count_comments_by_version(
        conn: &mut AsyncPgConnection,
        version_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::document_version_id.eq(version_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
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
    pub async fn count_comment_replies(
        conn: &mut AsyncPgConnection,
        parent_comment_id: Uuid,
    ) -> PgResult<i64> {
        use schema::document_comments::{self, dsl};

        let count = document_comments::table
            .filter(dsl::parent_comment_id.eq(parent_comment_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
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
    pub async fn find_recent_comments(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<DocumentComment>> {
        use schema::document_comments::{self, dsl};

        let seven_days_ago = OffsetDateTime::now_utc() - time::Duration::days(7);

        let comments = document_comments::table
            .filter(dsl::created_at.gt(seven_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(DocumentComment::as_select())
            .load(conn)
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

/// Comprehensive statistics for comments on a specific content target.
///
/// Provides insights into discussion engagement patterns, thread complexity,
/// and collaborative activity levels. These metrics help content managers
/// understand discussion dynamics and identify highly engaged content areas
/// that may benefit from additional moderation or community management.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentStats {
    /// Total number of comments including both top-level and replies
    pub total_count: i64,
    /// Number of top-level comments (excluding replies)
    pub top_level_count: i64,
    /// Number of reply comments (threaded responses)
    pub reply_count: i64,
}

impl CommentStats {
    /// Calculates the average depth of comment threads.
    ///
    /// Returns the ratio of replies to top-level comments, indicating
    /// discussion thread depth and engagement quality. Higher ratios
    /// suggest more interactive and detailed discussions.
    pub fn average_thread_depth(&self) -> f64 {
        if self.top_level_count == 0 {
            0.0
        } else {
            self.reply_count as f64 / self.top_level_count as f64
        }
    }

    /// Indicates whether there are active discussions on the content.
    ///
    /// Returns true if any comments exist, regardless of type,
    /// which can indicate content engagement and collaborative activity.
    pub fn has_active_discussions(&self) -> bool {
        self.total_count > 0
    }

    /// Calculates the reply rate as a percentage.
    ///
    /// Returns the percentage of total comments that are replies,
    /// indicating the conversational nature of discussions and
    /// engagement quality beyond initial comments.
    pub fn reply_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.reply_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Indicates whether discussions show deep engagement.
    ///
    /// Returns true if there are more replies than top-level comments,
    /// suggesting rich conversational engagement and detailed discussions.
    pub fn has_deep_engagement(&self) -> bool {
        self.reply_count > self.top_level_count
    }

    /// Indicates whether the content has generated substantial discussion.
    ///
    /// Returns true if the content has more than a threshold number of comments,
    /// suggesting significant community engagement and interest.
    pub fn has_substantial_discussion(&self) -> bool {
        self.total_count >= 10 // Configurable threshold
    }
}
