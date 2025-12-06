//! Document comment model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_comments;
use crate::types::constants::comment;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Document comment model representing user discussions about documents, files, or versions.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentComment {
    /// Unique comment identifier.
    pub id: Uuid,
    /// Reference to the parent document (mutually exclusive with file/version).
    pub document_id: Option<Uuid>,
    /// Reference to the parent document file (mutually exclusive with document/version).
    pub document_file_id: Option<Uuid>,
    /// Reference to the parent document version (mutually exclusive with document/file).
    pub document_version_id: Option<Uuid>,
    /// Reference to the account that authored this comment.
    pub account_id: Uuid,
    /// Parent comment for threaded replies (NULL for top-level comments).
    pub parent_comment_id: Option<Uuid>,
    /// Account being replied to (@mention).
    pub reply_to_account_id: Option<Uuid>,
    /// Comment text content.
    pub content: String,
    /// Additional comment metadata.
    pub metadata: serde_json::Value,
    /// Timestamp when the comment was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the comment was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the comment was soft-deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new document comment.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = document_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentComment {
    /// Document ID (mutually exclusive with file/version).
    pub document_id: Option<Uuid>,
    /// Document file ID (mutually exclusive with document/version).
    pub document_file_id: Option<Uuid>,
    /// Document version ID (mutually exclusive with document/file).
    pub document_version_id: Option<Uuid>,
    /// Account ID.
    pub account_id: Uuid,
    /// Parent comment ID for replies.
    pub parent_comment_id: Option<Uuid>,
    /// Reply to account ID (@mention).
    pub reply_to_account_id: Option<Uuid>,
    /// Comment content.
    pub content: String,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a document comment.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = document_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocumentComment {
    /// Comment content.
    pub content: Option<String>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Enum representing the target type of a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentTarget {
    /// Comment is on a document.
    Document,
    /// Comment is on a document file.
    File,
    /// Comment is on a document version.
    Version,
}

impl DocumentComment {
    /// Returns the comment content, or `None` if the comment is deleted.
    pub fn get_content(&self) -> Option<String> {
        if self.is_deleted() {
            None
        } else {
            Some(self.content.clone())
        }
    }

    /// Returns the target type of this comment.
    pub fn target_type(&self) -> CommentTarget {
        if self.document_id.is_some() {
            CommentTarget::Document
        } else if self.document_file_id.is_some() {
            CommentTarget::File
        } else {
            CommentTarget::Version
        }
    }

    /// Returns the target ID of this comment.
    pub fn target_id(&self) -> Uuid {
        self.document_id
            .or(self.document_file_id)
            .or(self.document_version_id)
            .expect("Comment must have exactly one target")
    }

    /// Returns whether this is a top-level comment (not a reply).
    pub fn is_top_level(&self) -> bool {
        self.parent_comment_id.is_none()
    }

    /// Returns whether this is a reply to another comment.
    pub fn is_reply(&self) -> bool {
        self.parent_comment_id.is_some()
    }

    /// Returns whether this comment mentions another account.
    pub fn has_mention(&self) -> bool {
        self.reply_to_account_id.is_some()
    }

    /// Returns whether this comment is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether this comment has been edited.
    pub fn is_edited(&self) -> bool {
        let duration = self.updated_at - self.created_at;
        duration.whole_seconds() > comment::EDIT_GRACE_PERIOD_SECONDS
    }

    /// Returns whether this comment is on a document.
    pub fn is_document_comment(&self) -> bool {
        self.document_id.is_some()
    }

    /// Returns whether this comment is on a file.
    pub fn is_file_comment(&self) -> bool {
        self.document_file_id.is_some()
    }

    /// Returns whether this comment is on a version.
    pub fn is_version_comment(&self) -> bool {
        self.document_version_id.is_some()
    }
}

impl NewDocumentComment {
    /// Creates a new comment on a document.
    pub fn for_document(document_id: Uuid, account_id: Uuid, content: String) -> Self {
        Self {
            document_id: Some(document_id),
            account_id,
            content,
            ..Default::default()
        }
    }

    /// Creates a new comment on a document file.
    pub fn for_file(document_file_id: Uuid, account_id: Uuid, content: String) -> Self {
        Self {
            document_file_id: Some(document_file_id),
            account_id,
            content,
            ..Default::default()
        }
    }

    /// Creates a new comment on a document version.
    pub fn for_version(document_version_id: Uuid, account_id: Uuid, content: String) -> Self {
        Self {
            document_version_id: Some(document_version_id),
            account_id,
            content,
            ..Default::default()
        }
    }

    /// Sets the parent comment ID for threaded replies.
    pub fn with_parent(mut self, parent_comment_id: Uuid) -> Self {
        self.parent_comment_id = Some(parent_comment_id);
        self
    }

    /// Sets the reply-to account ID for @mentions.
    pub fn with_reply_to(mut self, reply_to_account_id: Uuid) -> Self {
        self.reply_to_account_id = Some(reply_to_account_id);
        self
    }

    /// Sets custom metadata for the comment.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl HasCreatedAt for DocumentComment {
    fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

impl HasUpdatedAt for DocumentComment {
    fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

impl HasDeletedAt for DocumentComment {
    fn deleted_at(&self) -> Option<OffsetDateTime> {
        self.deleted_at
    }
}
