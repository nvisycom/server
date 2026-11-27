//! Workspace document import-origin model.
//!
//! The import origin for a document that was imported from a connection: the
//! connection and remote object key it came from. One row per imported document;
//! uploaded/generated documents have no row here.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_document_imports;

/// Data for recording a document's import origin.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_document_imports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDocumentImport {
    /// The imported document this origin describes.
    pub document_id: Uuid,
    /// Connection the document was imported from.
    pub connection_id: Uuid,
    /// Remote object key the document was imported from.
    pub source_key: String,
}
