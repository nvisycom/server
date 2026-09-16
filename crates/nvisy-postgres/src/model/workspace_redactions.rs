//! Workspace redaction model for `PostgreSQL` database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_redactions;

/// A redaction: one redact pass over a detection's analysis.
///
/// A detection can be redacted many times — each redact request may carry a
/// different set of reviewer edits — so each redaction is its own row. Its review
/// audit (the engine's findings after the reviewer edits were applied) is the blob
/// pointed at by `review_audit_blob_id`; the redacted output is a
/// [`WorkspaceDocument`] of kind `redacted`.
///
/// [`WorkspaceDocument`]: crate::model::WorkspaceDocument
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
    /// Redacted document (`kind = redacted`) this redaction produced. `None` only
    /// if the document was later hard-deleted.
    pub output_document_id: Option<Uuid>,
    /// Review-analysis (findings after reviewer edits) blob. `None` once the bytes
    /// are reclaimed on retention (the row is kept). A blob reference.
    pub review_audit_blob_id: Option<Uuid>,
    /// When the redaction was created.
    pub created_at: Timestamp,
    /// When the redaction was soft-deleted; `None` means live (the row is kept as
    /// a ledger record even after its bytes are reclaimed).
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace redaction.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_redactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceRedaction {
    /// Detection this redaction was produced from (required).
    pub detection_id: Uuid,
    /// Account that requested the redaction (required).
    pub account_id: Uuid,
    /// Redacted output document this redaction produced.
    pub output_document_id: Option<Uuid>,
    /// Review-analysis (findings after reviewer edits) blob this redaction produced.
    pub review_audit_blob_id: Option<Uuid>,
}

impl NewWorkspaceRedaction {
    /// A minimal redaction of `detection_id`, attributed to `account_id`, for
    /// tests. It carries no output document.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(detection_id: Uuid, account_id: Uuid) -> Self {
        Self {
            detection_id,
            account_id,
            output_document_id: None,
            review_audit_blob_id: None,
        }
    }
}
