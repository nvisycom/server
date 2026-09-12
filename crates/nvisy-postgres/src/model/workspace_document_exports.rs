//! Workspace document export-record model.
//!
//! Records that a document was exported to a connection, so scheduled export of
//! redacted outputs stays idempotent (a document already exported to a connection
//! is not pushed again). A document may be exported to more than one connection,
//! so the key is (document_id, connection_id).

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_document_exports;

/// A record that a document was exported to a connection.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_document_exports)]
#[diesel(primary_key(document_id, connection_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDocumentExport {
    /// The document that was exported.
    pub document_id: Uuid,
    /// Connection the document was exported to.
    pub connection_id: Uuid,
    /// Remote key the document was written to on the provider.
    pub remote_key: String,
    /// When the export was recorded.
    pub exported_at: Timestamp,
}

/// Data for recording a document export.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_document_exports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDocumentExport {
    /// The document that was exported.
    pub document_id: Uuid,
    /// Connection the document was exported to.
    pub connection_id: Uuid,
    /// Remote key the document was written to on the provider.
    pub remote_key: String,
}
