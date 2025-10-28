//! Document response types.

use nvisy_postgres::model::Document;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned when a document is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentResponse {
    /// ID of the document.
    pub document_id: Uuid,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
}

impl CreateDocumentResponse {
    /// Creates a new instance of [`CreateDocumentResponse`].
    pub fn new(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

impl From<Document> for CreateDocumentResponse {
    fn from(document: Document) -> Self {
        Self::new(document)
    }
}

/// Represents a document in a project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListDocumentsResponseItem {
    /// ID of the document.
    pub document_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
    pub display_name: String,
}

impl From<Document> for ListDocumentsResponseItem {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            account_id: document.account_id,
            display_name: document.display_name,
        }
    }
}

/// Response for listing all documents in a project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListDocumentsResponse {
    pub project_id: Uuid,
    pub documents: Vec<ListDocumentsResponseItem>,
}

impl ListDocumentsResponse {
    /// Returns a new [`ListDocumentsResponse`].
    pub fn new(project_id: Uuid, documents: Vec<ListDocumentsResponseItem>) -> Self {
        Self {
            project_id,
            documents,
        }
    }
}

/// Response for getting a single document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetDocumentResponse {
    /// ID of the document.
    pub id: Uuid,
    /// ID of the project that the document belongs to.
    pub project_id: Uuid,
    /// ID of the account that owns the document.
    pub account_id: Uuid,
    /// Display name of the document.
    pub display_name: String,
}

impl From<Document> for GetDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            id: document.id,
            project_id: document.project_id,
            account_id: document.account_id,
            display_name: document.display_name,
        }
    }
}

/// Response for updated document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocumentResponse {
    /// ID of the updated document.
    pub document_id: Uuid,
    /// Timestamp when the document was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the document was last updated.
    pub updated_at: OffsetDateTime,
}

impl UpdateDocumentResponse {
    /// Creates a new instance of `UpdateDocumentResponse`.
    pub fn new(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }
}

impl From<Document> for UpdateDocumentResponse {
    fn from(document: Document) -> Self {
        Self::new(document)
    }
}

/// Response returned after deleting a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDocumentResponse {
    pub document_id: Uuid,
    pub created_at: OffsetDateTime,
    pub deleted_at: OffsetDateTime,
}

impl From<Document> for DeleteDocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            document_id: document.id,
            created_at: document.created_at,
            deleted_at: document.deleted_at.unwrap_or_else(OffsetDateTime::now_utc),
        }
    }
}
