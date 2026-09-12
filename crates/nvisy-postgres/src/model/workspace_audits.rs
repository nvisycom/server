//! Workspace audit model for PostgreSQL database operations.
//!
//! An audit is the engine's findings set over a document, stored as bytes in a
//! [`Blob`](crate::model::Blob). Audits and reviews were once two file kinds;
//! they are the same entity, distinguished only by lineage:
//!
//! - a **base audit** is produced by a detection: `detection_id` set,
//!   `redaction_id` and `derived_from` NULL.
//! - a **review audit** is produced by a redaction applying reviewer edits:
//!   `detection_id` set (the detection it ultimately belongs to), plus
//!   `redaction_id` (the redaction that produced it) and `derived_from` (the base
//!   audit it was edited from).
//!
//! A database CHECK keeps `redaction_id` and `derived_from` set or NULL together.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_audits;

/// An audit: a findings set over a document, with detection/redaction lineage.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_audits)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceAudit {
    /// Unique audit identifier.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Blob holding the findings bytes.
    pub blob_id: Uuid,
    /// Detection whose analysis this audit belongs to (always set).
    pub detection_id: Uuid,
    /// Redaction that produced this review audit; `None` for a base audit.
    pub redaction_id: Option<Uuid>,
    /// Base audit this review was edited from; `None` for a base audit.
    pub derived_from: Option<Uuid>,
    /// When the audit was created.
    pub created_at: Timestamp,
}

/// Data for creating a new audit.
///
/// For a base audit leave `redaction_id` and `derived_from` `None`; for a review
/// audit set both. The database enforces that they agree.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_audits)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceAudit {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Blob holding the findings bytes (required).
    pub blob_id: Uuid,
    /// Detection this audit belongs to (required).
    pub detection_id: Uuid,
    /// Redaction that produced this review audit (set for a review audit).
    pub redaction_id: Option<Uuid>,
    /// Base audit this review was edited from (set for a review audit).
    pub derived_from: Option<Uuid>,
}

impl NewWorkspaceAudit {
    /// A base audit for `detection_id` backed by `blob_id`.
    pub fn base(workspace_id: Uuid, blob_id: Uuid, detection_id: Uuid) -> Self {
        Self {
            workspace_id,
            blob_id,
            detection_id,
            redaction_id: None,
            derived_from: None,
        }
    }

    /// A review audit for `redaction_id`, edited from the base audit
    /// `derived_from`.
    pub fn review(
        workspace_id: Uuid,
        blob_id: Uuid,
        detection_id: Uuid,
        redaction_id: Uuid,
        derived_from: Uuid,
    ) -> Self {
        Self {
            workspace_id,
            blob_id,
            detection_id,
            redaction_id: Some(redaction_id),
            derived_from: Some(derived_from),
        }
    }
}
