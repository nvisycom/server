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
    /// Timestamp when the document was created.
    pub created_at: Timestamp,
    /// Timestamp when the document was last updated.
    pub updated_at: Timestamp,
}

impl Document {
    /// Creates a Document response from a database model.
    pub fn from_model(document: model::Document) -> Self {
        Self {
            tags: document.tags(),
            document_id: document.id,
            workspace_id: document.workspace_id,
            account_id: document.account_id,
            display_name: document.display_name,
            description: document.description,
            status: document.status,
            created_at: document.created_at.into(),
            updated_at: document.updated_at.into(),
        }
    }

    /// Creates a list of Document responses from database models.
    pub fn from_models(models: Vec<model::Document>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}

/// Response for listing all documents in a workspace.
pub type Documents = Vec<Document>;
