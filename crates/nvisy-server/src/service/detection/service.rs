//! Detection enqueue service.
//!
//! The request-side counterpart to the [`DetectionWorker`](super::DetectionWorker):
//! publishes a run's detection to the `DetectionStream` work-queue and broadcasts
//! its status on the run's core-NATS subject. Injected into the create-run
//! handler so the handler stays thin and the NATS wiring lives in one place.

use nvisy_nats::NatsClient;
use nvisy_nats::stream::DetectionStream;
use nvisy_postgres::types::PipelineRunStatus;
use uuid::Uuid;

use super::job::{DetectionJob, RunStatusEvent, run_subject};
use crate::handler::Result;

/// Enqueues pipeline detection jobs and broadcasts run-status changes.
///
/// Cheaply cloneable (holds only a [`NatsClient`], which is `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you enqueue or broadcast with it"]
pub struct DetectionService {
    nats: NatsClient,
}

impl DetectionService {
    /// Creates a new [`DetectionService`].
    pub fn new(nats: NatsClient) -> Self {
        Self { nats }
    }

    /// Enqueues a run's detection onto the work-queue for the worker to pick up.
    pub async fn enqueue(&self, job: DetectionJob) -> Result<()> {
        let publisher = self
            .nats
            .event_publisher::<DetectionJob, DetectionStream>()
            .await?;
        publisher.publish(&job).await?;
        Ok(())
    }

    /// Broadcasts a run's status change on its core-NATS subject (best-effort;
    /// the run row is authoritative, so a dropped broadcast is recoverable).
    pub async fn broadcast_status(&self, run_id: Uuid, status: PipelineRunStatus) {
        let event = RunStatusEvent { run_id, status };
        if let Err(err) = self
            .nats
            .publish_broadcast(run_subject(run_id), &event)
            .await
        {
            tracing::debug!(
                target: "nvisy_server::service::detection",
                error = %err,
                "Failed to broadcast run status",
            );
        }
    }

    /// Subscribes to a run's status broadcasts, yielding each [`RunStatusEvent`].
    ///
    /// Used by the SSE endpoint to forward status changes to a watching client.
    pub async fn subscribe_status(
        &self,
        run_id: Uuid,
    ) -> Result<impl futures::Stream<Item = RunStatusEvent> + Send> {
        let stream = self
            .nats
            .subscribe_broadcast::<RunStatusEvent>(run_subject(run_id))
            .await?;
        Ok(stream)
    }
}
