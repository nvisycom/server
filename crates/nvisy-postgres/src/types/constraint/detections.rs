//! Detections table constraint violations.

use strum::EnumString;

/// Detections table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceDetectionConstraints {
    #[strum(serialize = "workspace_detections_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_detections_idempotency_key_length")]
    IdempotencyKeyLength,
    #[strum(serialize = "workspace_detections_idempotency_idx")]
    IdempotencyUnique,
}
