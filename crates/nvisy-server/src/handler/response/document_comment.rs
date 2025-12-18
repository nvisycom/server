//! Document comment response types.

use nvisy_postgres::model;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentComment {
    /// ID of the comment.
    pub comment_id: Uuid,
    /// ID of the document (if comment is on a document).
    pub document_id: Option<Uuid>,
    /// ID of the document file (if comment is on a file).
    pub document_file_id: Option<Uuid>,
    /// ID of the account that created the comment.
    pub account_id: Uuid,
    /// Parent comment ID for threaded replies.
    pub parent_comment_id: Option<Uuid>,
    /// Account being replied to (@mention).
    pub reply_to_account_id: Option<Uuid>,
    /// Comment text content.
    pub content: Option<String>,
    /// Timestamp when the comment was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the comment was last updated.
    pub updated_at: OffsetDateTime,
}

impl From<model::DocumentComment> for DocumentComment {
    fn from(comment: model::DocumentComment) -> Self {
        Self {
            comment_id: comment.id,
            document_id: comment.document_id,
            document_file_id: comment.document_file_id,
            account_id: comment.account_id,
            parent_comment_id: comment.parent_comment_id,
            reply_to_account_id: comment.reply_to_account_id,
            content: comment.get_content(),
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

/// Response for listing comments.
pub type DocumentComments = Vec<DocumentComment>;
