//! Workspace detection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_detections;
use crate::types::{DetectionMetadata, DetectionStatus, Json, PipelineTriggerType};

/// A detection: one analysis pass of a file through a pipeline.
///
/// Detect creates the detection and stores the engine's `Audit` in the object
/// store, keeping its file id here; the detection then stays `Complete` and can
/// be redacted any number of times (each redaction is its own row).
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDetection {
    /// Unique detection identifier.
    pub id: Uuid,
    /// Pipeline whose config drove the detection.
    pub pipeline_id: Uuid,
    /// Account the detection is attributed to (the user who started it, or the
    /// pipeline's creator for a system-initiated detection).
    pub account_id: Uuid,
    /// Source document the detection analyzes.
    pub input_file_id: Uuid,
    /// Audit file (`file_kind = audit`) holding the encrypted analysis. `None`
    /// until analysis writes it.
    pub audit_file_id: Option<Uuid>,
    /// Intermediates file (`file_kind = intermediate`) holding the encrypted
    /// enrichment (OCR layout, transcript). `None` until analysis writes it, and
    /// stays `None` for a document whose modality needs no enrichment.
    pub intermediates_file_id: Option<Uuid>,
    /// How the detection was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current detection status.
    pub status: DetectionStatus,
    /// Detect idempotency key (dedupes retries).
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Json<DetectionMetadata>,
    /// When a worker last claimed this detection. Acts as a lease: a redelivered
    /// job whose claim is still fresh is skipped, while a stale claim (a worker
    /// that died mid-analysis) can be re-claimed. `None` until first claimed.
    pub claimed_at: Option<Timestamp>,
    /// When the detection started.
    pub started_at: Timestamp,
    /// When the detection completed analysis.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace detection.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceDetection {
    /// Pipeline ID (required).
    pub pipeline_id: Uuid,
    /// Account the detection is attributed to (required).
    pub account_id: Uuid,
    /// Source document ID (required).
    pub input_file_id: Uuid,
    /// Audit file holding the encrypted analysis (set once analyzed).
    pub audit_file_id: Option<Uuid>,
    /// Intermediates file holding the encrypted enrichment (set once analyzed, if
    /// the document produced any).
    pub intermediates_file_id: Option<Uuid>,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<DetectionStatus>,
    /// Detect idempotency key.
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
}

/// Data for updating a workspace detection.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceDetection {
    /// Detection status.
    pub status: Option<DetectionStatus>,
    /// Audit file holding the encrypted analysis.
    pub audit_file_id: Option<Option<Uuid>>,
    /// Intermediates file holding the encrypted enrichment.
    pub intermediates_file_id: Option<Option<Uuid>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
    /// When a worker last claimed this detection (lease timestamp).
    pub claimed_at: Option<Option<Timestamp>>,
    /// When the detection completed analysis.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspaceDetection {
    /// Returns whether analysis is done and the detection is ready to redact.
    pub fn is_complete(&self) -> bool {
        self.status.is_complete()
    }
}
