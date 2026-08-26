//! Detection response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceDetection as DetectionModel;
use nvisy_postgres::query::DetectionFiles;
use nvisy_postgres::types::{
    DetectionId, DetectionMetadata, DetectionStatus, Handle, PipelineTriggerType,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a detection.
///
/// A detection is addressed by its own opaque id; the owning pipeline and
/// workspace slugs are carried for context. Redacted outputs are not here — a
/// detection produces many redactions, each fetched from its `redactions`
/// endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    /// Opaque identifier of the detection.
    pub id: DetectionId,
    /// Handle of the pipeline this detection belongs to.
    pub pipeline_slug: Handle,
    /// Handle of the workspace this detection belongs to.
    pub workspace_slug: Handle,
    /// Source document this detection analyzes.
    pub input_file_id: Uuid,
    /// Display name of the source document, for showing the detection without a
    /// separate file lookup. `None` if the file was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
    /// Account that triggered the detection.
    pub triggered_by: AccountRef,
    /// How the detection was triggered.
    pub trigger_type: PipelineTriggerType,
    /// Current detection status.
    ///
    /// The detections are available to fetch from the detection's `analysis`
    /// endpoint once this reaches `complete`.
    pub status: DetectionStatus,
    /// Human-readable failure reason, present only when the detection `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: DetectionMetadata,
    /// When the detection started.
    pub started_at: Timestamp,
    /// When the detection completed analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Paginated response for detections.
pub type DetectionsPage = Page<Detection>;

impl Detection {
    /// Creates a detection response from the database model, the slugs of its
    /// owning pipeline and workspace, the triggering account, and the resolved
    /// input file display name.
    pub fn from_model(
        detection: DetectionModel,
        pipeline_slug: Handle,
        workspace_slug: Handle,
        triggered_by: AccountRef,
        files: DetectionFiles,
    ) -> Self {
        // Surface the failure reason (written to metadata.error by the worker /
        // enqueue-failure path) as a dedicated field for a failed detection.
        let metadata = detection.metadata.or_default();
        let error = metadata.error.clone();

        Self {
            id: DetectionId::from_uuid(detection.id),
            pipeline_slug,
            workspace_slug,
            input_file_id: detection.input_file_id,
            input_file_name: files.input,
            triggered_by,
            trigger_type: detection.trigger_type,
            status: detection.status,
            error,
            metadata,
            started_at: detection.started_at.into(),
            completed_at: detection.completed_at.map(Into::into),
        }
    }
}
