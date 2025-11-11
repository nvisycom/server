//! Document response types.

use nvisy_postgres::model;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// ID of the document.
    pub document_id: Uuid,
    /// ID of the project that the document belongs to.
    pub project_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
    pub display_name: String,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
}

impl From<model::Document> for Document {
    fn from(document: model::Document) -> Self {
        Self {
            document_id: document.id,
            project_id: document.project_id,
            account_id: document.account_id,
            display_name: document.display_name,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

/// Response for listing all documents in a project.
pub type Documents = Vec<Document>;
