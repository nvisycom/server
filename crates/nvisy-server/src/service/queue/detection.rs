//! The request-side handle for the detection subsystem.

use nvisy_nats::stream::BroadcastStream;
use nvisy_postgres::types::DetectionStatus;
use uuid::Uuid;

use crate::response::Result;
use crate::service::Infra;
use crate::worker::Coordinator;
use crate::worker::detection::{
    DetectionJob, DetectionStatusEvent, broadcast_status, enqueue, subscribe_status,
};

/// Enqueues detection jobs and broadcasts detection-status changes.
///
/// Cheaply cloneable (holds the shared [`Infra`] clients and the [`Coordinator`],
/// all `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you enqueue or broadcast with it"]
pub struct DetectionQueue {
    infra: Infra,
    coordinator: Coordinator,
}

impl DetectionQueue {
    /// Creates a new [`DetectionQueue`].
    pub fn new(infra: Infra, coordinator: Coordinator) -> Self {
        Self { infra, coordinator }
    }

    /// Enqueues a detection's analysis onto the work-queue for the worker to pick
    /// up.
    pub async fn enqueue(&self, job: DetectionJob) -> Result<()> {
        enqueue(&self.infra, job).await
    }

    /// Wakes the detection-job outbox drainer so a just-committed job is drained
    /// immediately instead of waiting for the drainer's next timer tick. Call
    /// after the transaction that inserted the job row has committed. Best-effort
    /// and coalescing: the drainer's timer still covers a missed wake (a crash
    /// between commit and this call, or a job that landed on another instance).
    pub fn wake_drainer(&self) {
        self.coordinator.wake();
    }

    /// Broadcasts a detection's status change on its core-NATS subject
    /// (best-effort; the detection row is authoritative, so a dropped broadcast is
    /// recoverable).
    pub async fn broadcast_status(&self, detection_id: Uuid, status: DetectionStatus) {
        broadcast_status(&self.infra, detection_id, status).await;
    }

    /// Subscribes to a detection's status broadcasts, yielding each
    /// [`DetectionStatusEvent`].
    ///
    /// Used by the SSE endpoint to forward status changes to a watching client.
    pub async fn subscribe_status(
        &self,
        detection_id: Uuid,
    ) -> Result<BroadcastStream<DetectionStatusEvent>> {
        subscribe_status(&self.infra, detection_id).await
    }
}
