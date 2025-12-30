//! Document response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::DocumentStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a document with full details.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// ID of the document.
    pub document_id: Uuid,
    /// ID of the workspace that the document belongs to.
    pub workspace_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
    pub display_name: String,
    /// Description of the document.
    pub description: Option<String>,
    /// Tags associated with the document.
    pub tags: Vec<String>,
    /// Document status.
    pub status: DocumentStatus,
    /// File size in bytes.
    pub file_size: Option<i64>,
    /// MIME type of the document.
    pub mime_type: Option<String>,
    /// Timestamp when the document was created.
    pub created_at: Timestamp,
    /// Timestamp when the document was last updated.
    pub updated_at: Timestamp,
}

impl From<model::Document> for Document {
    fn from(document: model::Document) -> Self {
        Self {
            tags: document.tags(),

            document_id: document.id,
            workspace_id: document.workspace_id,
            account_id: document.account_id,
            display_name: document.display_name,
            description: document.description,
            status: document.status,
            file_size: None,
            mime_type: None,
            created_at: document.created_at.into(),
            updated_at: document.updated_at.into(),
        }
    }
}

/// Response for listing all documents in a workspace.
pub type Documents = Vec<Document>;

/// Document search results with relevance scoring.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResult {
    /// The matching document.
    pub document: Document,
    /// Relevance score (0.0 to 1.0).
    pub relevance_score: f32,
    /// Matching text snippets.
    pub snippets: Vec<String>,
}

/// Response for document search operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResults {
    /// Search results with scoring.
    pub results: Vec<DocumentSearchResult>,
    /// Total number of matches.
    pub total_matches: u64,
    /// Query that was executed.
    pub query: String,
    /// Time taken for search in milliseconds.
    pub search_time_ms: u32,
    /// Search suggestions.
    pub suggestions: Vec<String>,
}
