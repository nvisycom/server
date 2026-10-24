//! Workspace detection model for `PostgreSQL` database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_detections;
use crate::types::{DetectionMetadata, DetectionStatus, Json, RetentionOverride};

/// A detection: one analysis pass of a document against a set of policies.
///
/// Detect creates the detection and stores the engine's `Audit` (the base
/// analysis) in a blob pointed at by `audit_blob_id`; the detection then stays
/// `Complete` and can be redacted any number of times (each redaction is its own
/// row).
///
/// A detection is workspace-scoped directly and names the policies it runs against
/// at create time.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDetection {
    /// Unique detection identifier.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Account the detection is attributed to (the user who started it).
    pub account_id: Uuid,
    /// Source document the detection analyzes.
    pub input_document_id: Uuid,
    /// Intermediate blob holding the encrypted enrichment (OCR layout, transcript,
    /// tokenized text). `None` until analysis writes it, and stays `None` when the
    /// analysis ran no enricher. A blob reference, not a document.
    pub intermediate_blob_id: Option<Uuid>,
    /// Base-analysis (findings set) blob. `None` before analysis commits and once
    /// the bytes are reclaimed on retention (the row is kept). A blob reference.
    pub audit_blob_id: Option<Uuid>,
    /// Current detection status.
    pub status: DetectionStatus,
    /// Detect idempotency key (dedupes retries).
    pub idempotency_key: Option<String>,
    /// Retention override the detection ran under, snapshotted at create. Read on
    /// redact so output retention reflects the run. `None` falls back to the
    /// workspace retention baseline.
    pub retention_override: Option<Json<RetentionOverride>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Json<DetectionMetadata>,
    /// Accumulated reviewer edits (the engine's `EditSet`) layered over the base
    /// analysis at redact time, so manual corrections persist between redactions.
    /// Stored opaquely as JSON here (the engine type lives in the server layer);
    /// `None` means no edits yet.
    pub edit_set: Option<Json<serde_json::Value>>,
    /// Optimistic-concurrency version for `edit_set`, bumped on every edit write; an
    /// edit names the revision it read and the write proceeds only if it still matches.
    pub edit_set_rev: i32,
    /// When a worker last claimed this detection. Acts as a lease: a redelivered
    /// job whose claim is still fresh is skipped, while a stale claim (a worker
    /// that died mid-analysis) can be re-claimed. `None` until first claimed.
    pub claimed_at: Option<Timestamp>,
    /// When the detection started.
    pub started_at: Timestamp,
    /// When the detection completed analysis.
    pub completed_at: Option<Timestamp>,
    /// When the detection was soft-deleted; `None` means live (the row is kept as
    /// a ledger record even after its bytes are reclaimed).
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace detection.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDetection {
    /// Owning workspace (required).
    pub workspace_id: Uuid,
    /// Account the detection is attributed to (required).
    pub account_id: Uuid,
    /// Source document ID (required).
    pub input_document_id: Uuid,
    /// Intermediate blob holding the encrypted enrichment (set once analyzed, if
    /// the document produced any).
    pub intermediate_blob_id: Option<Uuid>,
    /// Initial status.
    pub status: Option<DetectionStatus>,
    /// Detect idempotency key.
    pub idempotency_key: Option<String>,
    /// Retention override the detection runs under, snapshotted from the detect
    /// request.
    pub retention_override: Option<Json<RetentionOverride>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
}

impl NewWorkspaceDetection {
    /// A minimal detection of `input_document_id` for tests. The status takes its
    /// database default (`pending`), so the detection starts unclaimed and claimable.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid, input_document_id: Uuid) -> Self {
        Self {
            workspace_id,
            account_id,
            input_document_id,
            ..Default::default()
        }
    }
}

/// Data for updating a workspace detection.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceDetection {
    /// Detection status.
    pub status: Option<DetectionStatus>,
    /// Intermediate blob holding the encrypted enrichment.
    pub intermediate_blob_id: Option<Option<Uuid>>,
    /// Base-analysis (findings set) blob, set when the analysis commits.
    pub audit_blob_id: Option<Option<Uuid>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
    /// Accumulated reviewer edits; outer `None` leaves it, inner `None` clears it.
    pub edit_set: Option<Option<Json<serde_json::Value>>>,
    /// When a worker last claimed this detection (lease timestamp).
    pub claimed_at: Option<Option<Timestamp>>,
    /// When the detection completed analysis.
    pub completed_at: Option<Option<Timestamp>>,
}
