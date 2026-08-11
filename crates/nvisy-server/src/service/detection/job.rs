//! Detection job and run-status event types.

use nvisy_engine::plan::ScopeParams;
use nvisy_postgres::types::PipelineRunStatus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A queued request to run detection for a pipeline run.
///
/// Published to the `DetectionStream` work-queue by the create-run handler and
/// consumed by the [`DetectionWorker`](super::DetectionWorker). The worker
/// re-loads the run, pipeline, file, and policies from the ids; only the
/// caller-supplied per-request scope, which is not otherwise persisted, travels
/// on the job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionJob {
    /// Workspace owning the run.
    pub workspace_id: Uuid,
    /// The run to analyze.
    pub run_id: Uuid,
    /// Caller-supplied per-request scope override, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeParams>,
}

/// A run's status change, broadcast on the core-NATS subject [`run_subject`].
///
/// Fan-out to any watching SSE connections; the run row in Postgres remains the
/// source of truth, so a missed broadcast is recoverable by re-reading the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusEvent {
    /// The run whose status changed.
    pub run_id: Uuid,
    /// The run's new status.
    pub status: PipelineRunStatus,
}

/// The core-NATS subject a run's status changes are broadcast on.
#[must_use]
pub fn run_subject(run_id: Uuid) -> String {
    format!("pipeline.runs.{run_id}.status")
}
