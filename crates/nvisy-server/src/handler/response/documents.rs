//! Document response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Represents a document with full details.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// Timestamp when the document was created.
    pub created_at: Timestamp,
    /// Timestamp when the document was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of documents.
pub type DocumentsPage = Page<Document>;

impl Document {
    pub fn from_model(document: model::Document) -> Self {
        Self {
            tags: document.tags(),
            document_id: document.id,
            workspace_id: document.workspace_id,
            account_id: document.account_id,
            display_name: document.display_name,
            description: document.description,
            created_at: document.created_at.into(),
            updated_at: document.updated_at.into(),
        }
    }
}
