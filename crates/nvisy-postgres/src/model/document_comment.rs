//! Document comment model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::document_comments;
use crate::types::constants::comment;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Document comment model representing user discussions on files.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentComment {
    /// Unique comment identifier.
    pub id: Uuid,
    /// Reference to the parent file.
    pub file_id: Uuid,
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
    pub created_at: Timestamp,
    /// Timestamp when the comment was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the comment was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new document comment.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = document_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentComment {
    /// File ID.
    pub file_id: Uuid,
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

impl DocumentComment {
    /// Returns the comment content, or `None` if the comment is deleted.
    pub fn get_content(&self) -> Option<String> {
        if self.is_deleted() {
            None
        } else {
            Some(self.content.clone())
        }
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
        let duration =
            jiff::Timestamp::from(self.updated_at) - jiff::Timestamp::from(self.created_at);
        duration.get_seconds() > comment::EDIT_GRACE_PERIOD_SECONDS
    }
}

impl NewDocumentComment {
    /// Creates a new comment on a file.
    pub fn for_file(file_id: Uuid, account_id: Uuid, content: String) -> Self {
        Self {
            file_id,
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
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for DocumentComment {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for DocumentComment {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
