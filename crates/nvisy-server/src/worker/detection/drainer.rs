//! Detection-job outbox handler.
//!
//! Relays each pending detection-job outbox row onto the detection work-queue,
//! so an analysis queued transactionally with its detection reaches the worker
//! even if the process crashes between the commit and the publish. This is the
//! relay half of the transactional outbox: the create-detection handler writes
//! the row in the detection's transaction, and the drainer relays it to NATS.

use nvisy_postgres::PgConn;
use nvisy_postgres::model::{UpdateWorkspaceDetection, WorkspaceDetectionJob};
use nvisy_postgres::query::WorkspaceDetectionRepository;
use nvisy_postgres::types::{DetectionMetadata, DetectionStatus, Json};
use uuid::Uuid;

use super::job::{DetectionJob, broadcast_status, enqueue};
use crate::response::Result;
use crate::service::Infra;
use crate::worker::outbox::{Handled, OutboxDrainer, OutboxHandler, relay};

/// The failure reason recorded on a detection whose job the drainer gave up
/// publishing, so the detection's terminal state explains why it never analyzed.
const DEAD_LETTER_REASON: &str = "Detection could not be queued for analysis";

/// Drains the detection-job outbox onto the detection work-queue.
pub type DetectionOutboxDrainer = OutboxDrainer<DetectionJobRelay>;

/// Relays detection jobs, and fails the detection of a job that never relays.
pub struct DetectionJobRelay {
    infra: Infra,
}

impl DetectionJobRelay {
    /// Creates a new [`DetectionJobRelay`].
    #[must_use]
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }
}

impl OutboxHandler for DetectionJobRelay {
    /// Detections failed by a dead-lettered job, so their `Failed` status is
    /// broadcast once the batch commits.
    type Done = Uuid;
    type Row = WorkspaceDetectionJob;

    const NAME: &'static str = "detection_outbox_drainer";

    async fn handle(&self, _conn: &mut PgConn, row: &WorkspaceDetectionJob) -> Handled<Uuid> {
        relay(row.id, &row.job, |job: DetectionJob| {
            enqueue(&self.infra, job)
        })
        .await
    }

    /// The job never published, so the worker will never drive its detection to
    /// a terminal state: fail the detection in the same transaction so it does
    /// not hang `Pending` forever. The guard on `Pending` makes it a no-op when a
    /// failed publish (e.g. a lost ack) did reach a worker that now owns the
    /// detection, and then no terminal status is announced.
    async fn dead_letter(
        &self,
        conn: &mut PgConn,
        row: &WorkspaceDetectionJob,
    ) -> Result<Option<Uuid>> {
        let metadata = DetectionMetadata {
            error: Some(DEAD_LETTER_REASON.to_owned()),
            ..Default::default()
        };
        let failed = conn
            .fail_pending_detection(
                row.detection_id,
                UpdateWorkspaceDetection {
                    metadata: Some(Json::encode(&metadata)),
                    ..Default::default()
                },
            )
            .await?;
        Ok(failed.then_some(row.detection_id))
    }

    /// Best-effort: the detection row is authoritative, so a dropped broadcast is
    /// recoverable by a re-read.
    async fn after_commit(&self, failed: Vec<Uuid>) {
        for detection_id in failed {
            broadcast_status(&self.infra, detection_id, DetectionStatus::Failed).await;
        }
    }
}
