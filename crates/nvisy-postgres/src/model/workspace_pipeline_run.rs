//! Workspace pipeline run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::pipeline_runs;
use crate::types::{PipelineRunStatus, PipelineTriggerType};

/// Workspace pipeline run model representing an execution instance of a pipeline.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePipelineRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Reference to the pipeline definition.
    pub pipeline_id: Uuid,
    /// Account that triggered the run (optional).
    pub account_id: Option<Uuid>,
    /// How the run was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current execution status.
    pub status: PipelineRunStatus,
    /// Pipeline definition snapshot at run time.
    pub definition_snapshot: serde_json::Value,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: serde_json::Value,
    /// Execution logs as JSON array.
    pub logs: serde_json::Value,
    /// When execution started.
    pub started_at: Timestamp,
    /// When execution completed.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace pipeline run.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspacePipelineRun {
    /// Pipeline ID (required).
    pub pipeline_id: Uuid,
    /// Account ID (optional).
    pub account_id: Option<Uuid>,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<PipelineRunStatus>,
    /// Definition snapshot.
    pub definition_snapshot: serde_json::Value,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<serde_json::Value>,
    /// Execution logs as JSON array.
    pub logs: Option<serde_json::Value>,
}

/// Data for updating a workspace pipeline run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspacePipelineRun {
    /// Execution status.
    pub status: Option<PipelineRunStatus>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<serde_json::Value>,
    /// Execution logs as JSON array.
    pub logs: Option<serde_json::Value>,
    /// When execution completed.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspacePipelineRun {
    /// Returns whether the run is queued.
    pub fn is_queued(&self) -> bool {
        self.status.is_queued()
    }

    /// Returns whether the run is currently running.
    pub fn is_running(&self) -> bool {
        self.status.is_running()
    }

    /// Returns whether the run completed successfully.
    pub fn is_completed(&self) -> bool {
        self.status.is_completed()
    }

    /// Returns whether the run failed.
    pub fn is_failed(&self) -> bool {
        self.status.is_failed()
    }

    /// Returns whether the run was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.status.is_cancelled()
    }

    /// Returns whether the run is still active (queued or running).
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Returns whether the run has finished (completed, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        self.status.is_finished()
    }

    /// Returns the duration of the run in seconds, if available.
    pub fn duration_seconds(&self) -> Option<f64> {
        let completed = self.completed_at?;
        let started_ts: jiff::Timestamp = self.started_at.into();
        let completed_ts: jiff::Timestamp = completed.into();
        Some(completed_ts.duration_since(started_ts).as_secs_f64())
    }

    /// Returns whether the run was manually triggered.
    pub fn is_manual(&self) -> bool {
        self.trigger_type.is_manual()
    }

    /// Returns whether the run was triggered automatically.
    pub fn is_automatic(&self) -> bool {
        self.trigger_type.is_automatic()
    }

    /// Returns whether the run can be retried.
    pub fn is_retriable(&self) -> bool {
        self.status.is_retriable()
    }

    /// Returns the steps from the definition snapshot.
    pub fn steps(&self) -> Option<&Vec<serde_json::Value>> {
        self.definition_snapshot.get("steps")?.as_array()
    }

    /// Returns the number of steps in the run.
    pub fn step_count(&self) -> usize {
        self.steps().map_or(0, |s| s.len())
    }

    /// Returns the logs as an array, if available.
    pub fn log_entries(&self) -> Option<&Vec<serde_json::Value>> {
        self.logs.as_array()
    }

    /// Returns the number of log entries.
    pub fn log_count(&self) -> usize {
        self.log_entries().map_or(0, |l| l.len())
    }
}
