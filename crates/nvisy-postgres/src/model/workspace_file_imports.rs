//! Workspace file import-origin model.
//!
//! The import origin for a file that was imported from a connection: the
//! connection and remote object key it came from. One row per imported file;
//! uploaded/generated files have no row here.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_file_imports;

/// Import origin for an imported file.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_file_imports)]
#[diesel(primary_key(file_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceFileImport {
    /// The imported file this origin describes.
    pub file_id: Uuid,
    /// Connection the file was imported from.
    pub connection_id: Uuid,
    /// Remote object key the file was imported from.
    pub source_key: String,
}

/// Data for recording a file's import origin.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_file_imports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceFileImport {
    /// The imported file this origin describes.
    pub file_id: Uuid,
    /// Connection the file was imported from.
    pub connection_id: Uuid,
    /// Remote object key the file was imported from.
    pub source_key: String,
}
