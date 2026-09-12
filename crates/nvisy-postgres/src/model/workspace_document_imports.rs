//! Workspace document import-origin model.
//!
//! The import origin for a document that was imported from a connection: the
//! connection and remote object key it came from. One row per imported document;
//! uploaded/generated documents have no row here.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_document_imports;

/// Import origin for an imported document.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_document_imports)]
#[diesel(primary_key(document_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDocumentImport {
    /// The imported document this origin describes.
    pub document_id: Uuid,
    /// Connection the document was imported from.
    pub connection_id: Uuid,
    /// Remote object key the document was imported from.
    pub source_key: String,
    /// When the document was imported.
    pub imported_at: Timestamp,
}

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
