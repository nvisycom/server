//! Pipeline runs table constraint violations.

use strum::EnumString;

/// Pipeline runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspacePipelineRunConstraints {
    #[strum(serialize = "workspace_pipeline_runs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_pipeline_runs_idempotency_key_length")]
    IdempotencyKeyLength,
    #[strum(serialize = "workspace_pipeline_runs_idempotency_idx")]
    IdempotencyUnique,
}
