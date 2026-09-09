//! Workspace file export-record model.
//!
//! Records that a file was exported to a connection, so scheduled export of
//! redacted outputs stays idempotent (a file already exported to a connection is
//! not pushed again). A file may be exported to more than one connection, so the
//! key is (file_id, connection_id).

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_file_exports;

/// A record that a file was exported to a connection.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_file_exports)]
#[diesel(primary_key(file_id, connection_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct WorkspaceFileExport {
    /// The workspace file that was exported.
    pub file_id: Uuid,
    /// Connection the file was exported to.
    pub connection_id: Uuid,
    /// Remote key the file was written to on the provider.
    pub remote_key: String,
    /// When the export was recorded.
    pub exported_at: Timestamp,
}

/// Data for recording a file export.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_file_exports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceFileExport {
    /// The workspace file that was exported.
    pub file_id: Uuid,
    /// Connection the file was exported to.
    pub connection_id: Uuid,
    /// Remote key the file was written to on the provider.
    pub remote_key: String,
}
