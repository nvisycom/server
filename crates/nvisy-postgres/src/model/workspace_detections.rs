//! Workspace detection model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_detections;
use crate::types::{
    DetectionMetadata, DetectionStatus, Json, PipelineTriggerType, RetentionOverride,
};

/// A detection: one analysis pass of a document, optionally through a pipeline.
///
/// Detect creates the detection and stores the engine's `Audit` as a
/// [`WorkspaceAudit`](crate::model::WorkspaceAudit) row (the base audit) pointing
/// back via `detection_id`; the detection then stays `Complete` and can be
/// redacted any number of times (each redaction is its own row).
///
/// A detection is workspace-scoped directly. A pipeline detection names the
/// pipeline whose config drove it; an ad-hoc detection names its policies at
/// create time and carries no pipeline.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_detections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDetection {
    /// Unique detection identifier.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Pipeline whose config drove the detection. `None` for an ad-hoc detection
    /// or once the pipeline it ran through has been deleted.
    pub pipeline_id: Option<Uuid>,
    /// Account the detection is attributed to (the user who started it, or the
    /// pipeline's creator for a system-initiated detection).
    pub account_id: Uuid,
    /// Source document the detection analyzes.
    pub input_document_id: Uuid,
    /// Intermediate blob holding the encrypted enrichment (OCR layout, transcript,
    /// tokenized text). `None` until analysis writes it, and stays `None` when the
    /// analysis ran no enricher. A blob reference, not a document.
    pub intermediate_blob_id: Option<Uuid>,
    /// How the detection was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current detection status.
    pub status: DetectionStatus,
    /// Detect idempotency key (dedupes retries).
    pub idempotency_key: Option<String>,
    /// Retention override the detection ran under, snapshotted at create. Read on
    /// redact so output retention reflects the run, not a since-edited pipeline.
    /// `None` falls back to the workspace retention baseline.
    pub retention_override: Option<Json<RetentionOverride>>,
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
#[must_use]
pub struct NewWorkspaceDetection {
    /// Owning workspace (required).
    pub workspace_id: Uuid,
    /// Pipeline the detection runs through; `None` for an ad-hoc detection.
    pub pipeline_id: Option<Uuid>,
    /// Account the detection is attributed to (required).
    pub account_id: Uuid,
    /// Source document ID (required).
    pub input_document_id: Uuid,
    /// Intermediate blob holding the encrypted enrichment (set once analyzed, if
    /// the document produced any).
    pub intermediate_blob_id: Option<Uuid>,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<DetectionStatus>,
    /// Detect idempotency key.
    pub idempotency_key: Option<String>,
    /// Retention override the detection runs under, snapshotted from its pipeline
    /// or supplied directly for an ad-hoc detection.
    pub retention_override: Option<Json<RetentionOverride>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
}

impl NewWorkspaceDetection {
    /// A minimal detection of `input_document_id` through `pipeline_id`, for
    /// tests. The trigger and status take their database defaults (`user`,
    /// `pending`), so the detection starts unclaimed and claimable.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(
        workspace_id: Uuid,
        pipeline_id: Uuid,
        account_id: Uuid,
        input_document_id: Uuid,
    ) -> Self {
        Self {
            workspace_id,
            pipeline_id: Some(pipeline_id),
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
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<DetectionMetadata>>,
    /// When a worker last claimed this detection (lease timestamp).
    pub claimed_at: Option<Option<Timestamp>>,
    /// When the detection completed analysis.
    pub completed_at: Option<Option<Timestamp>>,
}
