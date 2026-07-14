//! Workspace pipeline run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_pipeline_runs;
use crate::types::{PipelineRunStatus, PipelineTriggerType};

/// A detect/redact run: one analysis of a file through a pipeline.
///
/// Detect creates the run and stores the engine's `AnalyzedDocument` in the
/// object store, keeping its key here; the run then awaits reviewer
/// verification before redact fetches it back and consumes it.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePipelineRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Pipeline whose config drove the run.
    pub pipeline_id: Uuid,
    /// File the run analyzes / redacts.
    pub file_id: Uuid,
    /// Account that triggered the run (optional).
    pub account_id: Option<Uuid>,
    /// How the run was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current run status.
    pub status: PipelineRunStatus,
    /// Human-facing sequence number within the pipeline (assigned at insert).
    pub run_number: i32,
    /// Object-store key for the encrypted `AnalyzedDocument` held between detect
    /// and redact. `None` until analysis writes it.
    pub analyzed_document_key: Option<String>,
    /// Detect idempotency key (dedupes retries).
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: serde_json::Value,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run completed.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace pipeline run.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspacePipelineRun {
    /// Pipeline ID (required).
    pub pipeline_id: Uuid,
    /// File ID (required).
    pub file_id: Uuid,
    /// Account ID (optional).
    pub account_id: Option<Uuid>,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<PipelineRunStatus>,
    /// Object-store key for the encrypted analysis result (set once analyzed).
    pub analyzed_document_key: Option<String>,
    /// Detect idempotency key.
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a workspace pipeline run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspacePipelineRun {
    /// Run status.
    pub status: Option<PipelineRunStatus>,
    /// Object-store key for the encrypted analysis result.
    pub analyzed_document_key: Option<Option<String>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<serde_json::Value>,
    /// When the run completed.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspacePipelineRun {
    /// Returns whether detection is in progress.
    pub fn is_running(&self) -> bool {
        self.status.is_running()
    }

    /// Returns whether detection is done and the run awaits verification.
    pub fn is_analyzed(&self) -> bool {
        self.status.is_analyzed()
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

    /// Returns whether the run is still active (running or awaiting review).
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
}
