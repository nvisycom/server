//! Detection job and detection-status event types, and the NATS enqueue and
//! status-broadcast primitives shared by the request-side [`DetectionQueue`] and
//! the background [`DetectionWorker`] and [`DetectionOutboxDrainer`].
//!
//! [`DetectionQueue`]: crate::service::DetectionQueue
//! [`DetectionWorker`]: super::DetectionWorker
//! [`DetectionOutboxDrainer`]: super::DetectionOutboxDrainer

use elide_pipeline::provider::DocumentContext;
use nvisy_nats::stream::BroadcastStream;
use nvisy_postgres::types::DetectionStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::response::Result;
use crate::service::Infra;

/// Tracing target for detection enqueue and broadcast primitives.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// The detection `JetStream` work-queue, carrying [`DetectionJob`] payloads.
pub type DetectionStream = nvisy_nats::stream::DetectionStream<DetectionJob>;

/// A queued request to run a detection.
///
/// Published to the `DetectionStream` work-queue by the create-detection handler
/// and consumed by the [`DetectionWorker`]. The worker re-loads the detection,
/// file, and (for a pipeline detection) the pipeline and its policies from the
/// ids; the caller-supplied per-request scope and, for an ad-hoc detection, the
/// policy ids it named — neither persisted on the detection — travel on the job.
///
/// [`DetectionWorker`]: super::DetectionWorker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionJob {
    /// Workspace owning the detection.
    pub workspace_id: Uuid,
    /// The detection to analyze.
    pub detection_id: Uuid,
    /// Caller-supplied per-request scope override, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
    /// For an ad-hoc detection (no pipeline), the policy ids it runs against;
    /// empty for a pipeline detection, whose policies come from the pipeline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_ids: Vec<Uuid>,
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

/// Publishes a detection job onto the `DetectionStream` work-queue for the worker
/// to pick up.
///
/// # Errors
///
/// A NATS error if publishing the job to the work-queue fails.
pub async fn enqueue(infra: &Infra, job: DetectionJob) -> Result<()> {
    let publisher = infra.nats.event_publisher::<DetectionStream>();
    publisher.publish(&job).await?;
    Ok(())
}

/// Broadcasts a detection's status change on its core-NATS subject (best-effort;
/// the detection row is authoritative, so a dropped broadcast is recoverable).
pub async fn broadcast_status(infra: &Infra, detection_id: Uuid, status: DetectionStatus) {
    let event = DetectionStatusEvent {
        detection_id,
        status,
    };
    if let Err(err) = infra
        .nats
        .publish_broadcast(detection_subject(detection_id), &event)
        .await
    {
        tracing::debug!(
            target: TRACING_TARGET,
            error = %err,
            "Failed to broadcast detection status",
        );
    }
}

/// Subscribes to a detection's status broadcasts, yielding each
/// [`DetectionStatusEvent`]. Used by the SSE endpoint to forward status changes to
/// a watching client.
///
/// # Errors
///
/// A NATS error if the broadcast subscription cannot be established.
pub async fn subscribe_status(
    infra: &Infra,
    detection_id: Uuid,
) -> Result<BroadcastStream<DetectionStatusEvent>> {
    let stream = infra
        .nats
        .subscribe_broadcast::<DetectionStatusEvent>(detection_subject(detection_id))
        .await?;
    Ok(stream)
}
