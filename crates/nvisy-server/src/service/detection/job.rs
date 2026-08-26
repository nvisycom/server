//! Detection job and detection-status event types.

use elide_pipeline::DocumentContext;
use nvisy_postgres::types::DetectionStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A queued request to run a detection.
///
/// Published to the `DetectionStream` work-queue by the create-detection handler
/// and consumed by the [`DetectionWorker`](super::DetectionWorker). The worker
/// re-loads the detection, pipeline, file, and policies from the ids; only the
/// caller-supplied per-request scope, which is not otherwise persisted, travels
/// on the job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionJob {
    /// Workspace owning the detection.
    pub workspace_id: Uuid,
    /// The detection to analyze.
    pub detection_id: Uuid,
    /// Caller-supplied per-request scope override, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
}

/// A detection's status change, broadcast on the core-NATS subject
/// [`detection_subject`].
///
/// Fan-out to any watching SSE connections; the detection row in Postgres remains
/// the source of truth, so a missed broadcast is recoverable by re-reading it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionStatusEvent {
    /// The detection whose status changed.
    pub detection_id: Uuid,
    /// The detection's new status.
    pub status: DetectionStatus,
}

/// The core-NATS subject a detection's status changes are broadcast on.
#[must_use]
pub fn detection_subject(detection_id: Uuid) -> String {
    format!("pipeline.detections.{detection_id}.status")
}
