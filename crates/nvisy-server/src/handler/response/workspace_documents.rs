//! Document response types.

use jiff::Timestamp;
use nvisy_postgres::model::{Blob, WorkspaceDocument as DocumentModel};
use nvisy_postgres::types::{DocumentKind, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Represents a document in responses.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocument {
    /// Unique document identifier.
    pub id: Uuid,
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Display name.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without dot). Owned by the document.
    pub extension: String,
    /// Size in bytes. Resolved from the backing blob.
    pub size: i64,
    /// Lowercase hex-encoded SHA-256 of the document's plaintext content. Resolved
    /// from the backing blob.
    pub hash: String,
    /// The document's role (original or redacted).
    pub kind: DocumentKind,
    /// Account that uploaded/created the document.
    pub uploaded_by: AccountRef,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl WorkspaceDocument {
    /// Builds the response from a document and its backing blob (which carries the
    /// content-addressed fields: size, hash).
    pub fn from_model(
        document: DocumentModel,
        blob: &Blob,
        workspace_id: Uuid,
        workspace_handle: Handle,
        uploaded_by: AccountRef,
    ) -> Self {
        Self {
            id: document.id,
            workspace_id,
            workspace_handle,
            display_name: document.display_name,
            original_filename: document.original_filename,
            extension: document.file_extension.clone(),
            size: blob.file_size_bytes,
            hash: hex::encode(&blob.content_hash),
            kind: document.kind,
            uploaded_by,
            created_at: document.created_at.into(),
            updated_at: document.updated_at.into(),
        }
    }
}

/// Full document metadata, returned by the read and update endpoints.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocumentDetails {
    /// Standard file fields, flattened to preserve the existing response shape.
    #[serde(flatten)]
    pub document: WorkspaceDocument,
    /// Custom metadata stored on the document.
    pub metadata: serde_json::Value,
}

impl WorkspaceDocumentDetails {
    /// Builds a detail response including the stored custom metadata.
    pub fn from_model(
        mut document: DocumentModel,
        blob: &Blob,
        workspace_id: Uuid,
        workspace_handle: Handle,
        uploaded_by: AccountRef,
    ) -> Self {
        let metadata = std::mem::take(&mut document.metadata);
        Self {
            document: WorkspaceDocument::from_model(
                document,
                blob,
                workspace_id,
                workspace_handle,
                uploaded_by,
            ),
            metadata,
        }
    }
}

/// Result of a bulk document deletion.
///
/// The deletion is idempotent: `deleted` holds the ids that resolved to live
/// documents in the workspace and were removed, and `skipped` holds the requested
/// ids that did not — unknown, already deleted, in another workspace, or held by
/// an in-progress detection that still needs the document.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedDocuments {
    /// Ids that were deleted.
    pub deleted: Vec<Uuid>,
    /// Requested ids that were skipped: unknown, already deleted, in another
    /// workspace, or held by an in-progress detection.
    pub skipped: Vec<Uuid>,
}

/// Response for document uploads (simple list without pagination).
pub type WorkspaceDocuments = Vec<WorkspaceDocument>;

/// Paginated response for document listing.
pub type WorkspaceDocumentsPage = Page<WorkspaceDocument>;

#[cfg(test)]
mod tests {
    use nvisy_postgres::model::UpdateWorkspaceDocument;
    use nvisy_postgres::query::WorkspaceDocumentRepository;
    use nvisy_postgres::test_util::TestDatabase;

    use super::*;

    #[tokio::test]
    async fn document_details_return_stored_custom_metadata_at_the_top_level() {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await.unwrap();
        let metadata = serde_json::json!({"tags": ["confidential"], "source": "client"});
        conn.update_workspace_document(
            seeded.document_id,
            UpdateWorkspaceDocument {
                metadata: Some(metadata.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let found = conn
            .find_document_in_workspace_with_creator(seeded.workspace_id, seeded.document_id)
            .await
            .unwrap()
            .unwrap();
        let details = WorkspaceDocumentDetails::from_model(
            found.item.document,
            &found.item.blob,
            seeded.workspace_id,
            "test-workspace".parse().unwrap(),
            found.account.into(),
        );
        let json = serde_json::to_value(details).unwrap();
        assert_eq!(json["id"], seeded.document_id.to_string());
        assert_eq!(json["metadata"], metadata);
        assert!(
            json.get("document").is_none(),
            "file fields must remain at the top level"
        );
    }
}
