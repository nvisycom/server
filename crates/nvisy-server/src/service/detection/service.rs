//! Detection enqueue service.
//!
//! The request-side counterpart to the [`DetectionWorker`](super::DetectionWorker):
//! publishes a detection's analysis to the `DetectionStream` work-queue and
//! broadcasts its status on the detection's core-NATS subject. Injected into the
//! create-detection handler so the handler stays thin and the NATS wiring lives in
//! one place.

use nvisy_nats::stream::{BroadcastStream, DetectionStream};
use nvisy_postgres::types::DetectionStatus;
use uuid::Uuid;

use super::job::{DetectionJob, DetectionStatusEvent, detection_subject};
use crate::handler::Result;
use crate::service::Infra;

/// Enqueues detection jobs and broadcasts detection-status changes.
///
/// Cheaply cloneable (holds the shared [`Infra`] clients, all `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you enqueue or broadcast with it"]
pub struct DetectionQueue {
    infra: Infra,
}

impl DetectionQueue {
    /// Creates a new [`DetectionQueue`].
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Enqueues a detection's analysis onto the work-queue for the worker to pick
    /// up.
    pub async fn enqueue(&self, job: DetectionJob) -> Result<()> {
        let publisher = self
            .infra
            .nats
            .event_publisher::<DetectionJob, DetectionStream>()
            .await?;
        publisher.publish(&job).await?;
        Ok(())
    }

    /// Broadcasts a detection's status change on its core-NATS subject
    /// (best-effort; the detection row is authoritative, so a dropped broadcast is
    /// recoverable).
    pub async fn broadcast_status(&self, detection_id: Uuid, status: DetectionStatus) {
        let event = DetectionStatusEvent {
            detection_id,
            status,
        };
        if let Err(err) = self
            .infra
            .nats
            .publish_broadcast(detection_subject(detection_id), &event)
            .await
        {
            tracing::debug!(
                target: "nvisy_server::service::detection",
                error = %err,
                "Failed to broadcast detection status",
            );
        }
    }

    /// Subscribes to a detection's status broadcasts, yielding each
    /// [`DetectionStatusEvent`].
    ///
    /// Used by the SSE endpoint to forward status changes to a watching client.
    pub async fn subscribe_status(
        &self,
        detection_id: Uuid,
    ) -> Result<BroadcastStream<DetectionStatusEvent>> {
        let stream = self
            .infra
            .nats
            .subscribe_broadcast::<DetectionStatusEvent>(detection_subject(detection_id))
            .await?;
        Ok(stream)
    }
}
