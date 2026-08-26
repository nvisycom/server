//! Workspace redaction model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_redactions;

/// A redaction: one redact pass over a detection's analysis.
///
/// A detection can be redacted many times — each redact request may carry a
/// different set of reviewer edits — so each redaction is its own row owning the
/// review audit it applied and the redacted document it produced.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_redactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceRedaction {
    /// Unique redaction identifier.
    pub id: Uuid,
    /// Detection this redaction was produced from.
    pub detection_id: Uuid,
    /// Account that requested the redaction.
    pub account_id: Uuid,
    /// Review audit (`file_kind = review`) recording the applied edits and the
    /// redaction outcome. `None` only if the file was later hard-deleted.
    pub review_file_id: Option<Uuid>,
    /// Redacted document (`file_kind = redacted`) this redaction produced.
    /// `None` only if the file was later hard-deleted.
    pub output_file_id: Option<Uuid>,
    /// When the redaction was created.
    pub created_at: Timestamp,
}

/// Data for creating a new workspace redaction.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_redactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceRedaction {
    /// Detection this redaction was produced from (required).
    pub detection_id: Uuid,
    /// Account that requested the redaction (required).
    pub account_id: Uuid,
    /// Review audit holding the applied edits and outcome.
    pub review_file_id: Option<Uuid>,
    /// Redacted output document this redaction produced.
    pub output_file_id: Option<Uuid>,
}
