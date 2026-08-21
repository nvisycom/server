//! Workspace pipeline run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_pipeline_runs;
use crate::types::{Json, PipelineRunStatus, PipelineTriggerType, RunMetadata};

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
    /// Account the run is attributed to (the user who started it, or the
    /// pipeline's creator for a system-initiated run).
    pub account_id: Uuid,
    /// Source document the run analyzes / redacts.
    pub input_file_id: Uuid,
    /// Audit file (`file_kind = audit`) holding the encrypted analysis between
    /// detect and redact. `None` until analysis writes it.
    pub audit_file_id: Option<Uuid>,
    /// Redacted document (`file_kind = redacted`) produced by redact. `None`
    /// until the run completes.
    pub output_file_id: Option<Uuid>,
    /// How the run was initiated.
    pub trigger_type: PipelineTriggerType,
    /// Current run status.
    pub status: PipelineRunStatus,
    /// Detect idempotency key (dedupes retries).
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Json<RunMetadata>,
    /// When a worker last claimed this run for detection. Acts as a lease: a
    /// redelivered job whose claim is still fresh is skipped, while a stale
    /// claim (a worker that died mid-analysis) can be re-claimed. `None` until
    /// first claimed.
    pub claimed_at: Option<Timestamp>,
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
    /// Account the run is attributed to (required).
    pub account_id: Uuid,
    /// Source document ID (required).
    pub input_file_id: Uuid,
    /// Audit file holding the encrypted analysis (set once analyzed).
    pub audit_file_id: Option<Uuid>,
    /// Redacted output file (set once completed).
    pub output_file_id: Option<Uuid>,
    /// Trigger type.
    pub trigger_type: Option<PipelineTriggerType>,
    /// Initial status.
    pub status: Option<PipelineRunStatus>,
    /// Detect idempotency key.
    pub idempotency_key: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<RunMetadata>>,
}

/// Data for updating a workspace pipeline run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_pipeline_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspacePipelineRun {
    /// Run status.
    pub status: Option<PipelineRunStatus>,
    /// Audit file holding the encrypted analysis.
    pub audit_file_id: Option<Option<Uuid>>,
    /// Redacted output file produced by redact.
    pub output_file_id: Option<Option<Uuid>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<Json<RunMetadata>>,
    /// When a worker last claimed this run for detection (lease timestamp).
    pub claimed_at: Option<Option<Timestamp>>,
    /// When the run completed.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspacePipelineRun {
    /// Returns whether detection is done and the run awaits verification.
    pub fn is_analyzed(&self) -> bool {
        self.status.is_analyzed()
    }
}
