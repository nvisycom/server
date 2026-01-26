//! Pipeline run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::pipeline_runs;
use crate::types::{HasCreatedAt, PipelineRunStatus, PipelineTriggerType};

/// Pipeline run model representing an execution instance of a pipeline.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PipelineRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Reference to the pipeline definition.
    pub pipeline_id: Uuid,
    /// Reference to the workspace.
    pub workspace_id: Uuid,
    /// Account that triggered the run.
    pub account_id: Uuid,
    /// How the run was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current execution status.
    pub status: PipelineRunStatus,
    /// Runtime input configuration.
    pub input_config: serde_json::Value,
    /// Runtime output configuration.
    pub output_config: serde_json::Value,
    /// Pipeline definition snapshot at run time.
    pub definition_snapshot: serde_json::Value,
    /// Error details if run failed.
    pub error: Option<serde_json::Value>,
    /// Run metrics (duration, resources, etc.).
    pub metrics: serde_json::Value,
    /// When execution started.
    pub started_at: Option<Timestamp>,
    /// When execution completed.
    pub completed_at: Option<Timestamp>,
    /// When run was created/queued.
    pub created_at: Timestamp,
}

/// Data for creating a new pipeline run.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewPipelineRun {
    /// Pipeline ID (required).
    pub pipeline_id: Uuid,
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<PipelineRunStatus>,
    /// Input configuration.
    pub input_config: Option<serde_json::Value>,
    /// Output configuration.
    pub output_config: Option<serde_json::Value>,
    /// Definition snapshot.
    pub definition_snapshot: serde_json::Value,
    /// Metrics.
    pub metrics: Option<serde_json::Value>,
}

/// Data for updating a pipeline run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdatePipelineRun {
    /// Execution status.
    pub status: Option<PipelineRunStatus>,
    /// Output configuration.
    pub output_config: Option<serde_json::Value>,
    /// Error details.
    pub error: Option<Option<serde_json::Value>>,
    /// Metrics.
    pub metrics: Option<serde_json::Value>,
    /// When execution started.
    pub started_at: Option<Option<Timestamp>>,
    /// When execution completed.
    pub completed_at: Option<Option<Timestamp>>,
}

impl PipelineRun {
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

    /// Returns whether the run has an error.
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Returns the error message if present.
    pub fn error_message(&self) -> Option<&str> {
        self.error
            .as_ref()
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
    }

    /// Returns the duration of the run in seconds, if available.
    pub fn duration_seconds(&self) -> Option<f64> {
        let started = self.started_at?;
        let completed = self.completed_at?;
        let started_ts: jiff::Timestamp = started.into();
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
}

impl HasCreatedAt for PipelineRun {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}
