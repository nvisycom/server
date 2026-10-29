//! `WorkspaceDetection` response types.

use elide_pipeline::entity::EditSet;
use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceDetection as DetectionModel;
use nvisy_postgres::query::DetectionDocuments;
use nvisy_postgres::types::{DetectionId, DetectionMetadata, DetectionStatus, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a detection.
///
/// A detection is addressed by its own opaque id; the workspace handle is carried
/// for context. Redacted outputs are not here — a detection produces many
/// redactions, each fetched from its `redactions` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetection {
    /// Opaque identifier of the detection.
    pub id: DetectionId,
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Source document this detection analyzes.
    pub input_document_id: Uuid,
    /// Display name of the source document, for showing the detection without a
    /// separate file lookup. `None` if the file was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_document_name: Option<String>,
    /// Account that triggered the detection.
    pub triggered_by: AccountRef,
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
    /// The accumulated reviewer edits applied over the base analysis at redact time,
    /// so a client can display and revise them. Absent when the detection has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditSet>,
    /// Optimistic-concurrency version of `edits`; echo it back on an edit PATCH so a
    /// stale write is rejected rather than overwriting a concurrent edit.
    pub edit_set_rev: i32,
    /// When the detection started.
    pub started_at: Timestamp,
    /// When the detection completed analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Compact detection list item. Full metadata and reviewer edits are available
/// from the individual detection endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionSummary {
    /// Detection identifier.
    pub id: DetectionId,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Workspace handle for display.
    pub workspace_handle: Handle,
    /// Input document identifier.
    pub input_document_id: Uuid,
    /// Input display name, omitted if the document was deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_document_name: Option<String>,
    /// Account that started the detection.
    pub triggered_by: AccountRef,
    /// Current execution status.
    pub status: DetectionStatus,
    /// Failure reason when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the detection started.
    pub started_at: Timestamp,
    /// When analysis finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    /// Whether intermediate bytes are retained.
    pub has_intermediate: bool,
    /// Number of live redacted output files in the artifacts view.
    pub redacted_output_count: i64,
}

impl WorkspaceDetectionSummary {
    /// Builds a compact response from a list query's projection.
    pub fn from_row(
        row: nvisy_postgres::query::DetectionListRow,
        workspace_id: Uuid,
        workspace_handle: Handle,
    ) -> Self {
        Self {
            id: DetectionId::from_uuid(row.detection.id),
            workspace_id,
            workspace_handle,
            input_document_id: row.detection.input_document_id,
            input_document_name: row.input_document_name,
            triggered_by: row.account.into(),
            status: row.detection.status,
            error: row.detection.error,
            started_at: row.detection.started_at.into(),
            completed_at: row.detection.completed_at.map(Into::into),
            has_intermediate: row.detection.has_intermediate,
            redacted_output_count: row.detection.redacted_output_count,
        }
    }
}

/// Paginated response containing compact detection summaries.
pub type WorkspaceDetectionsPage = Page<WorkspaceDetectionSummary>;

impl WorkspaceDetection {
    /// Creates a detection response from the database model, the owning workspace
    /// id and handle, the triggering account, and the resolved input file display
    /// name.
    pub fn from_model(
        detection: &DetectionModel,
        workspace_id: Uuid,
        workspace_handle: Handle,
        triggered_by: AccountRef,
        files: DetectionDocuments,
    ) -> Self {
        // Surface the failure reason (written to metadata.error by the worker /
        // enqueue-failure path) as a dedicated field for a failed detection.
        let metadata = detection.metadata.or_default();
        let error = metadata.error.clone();
        // The edit-set is stored opaquely as JSON; present it typed when it decodes,
        // and omit it (rather than surface a raw blob) if a stored shape no longer does.
        let edits = detection
            .edit_set
            .as_ref()
            .and_then(|stored| serde_json::from_value(stored.as_value().clone()).ok());

        Self {
            id: DetectionId::from_uuid(detection.id),
            workspace_id,
            workspace_handle,
            input_document_id: detection.input_document_id,
            input_document_name: files.input,
            triggered_by,
            status: detection.status,
            error,
            metadata,
            edits,
            edit_set_rev: detection.edit_set_rev,
            started_at: detection.started_at.into(),
            completed_at: detection.completed_at.map(Into::into),
        }
    }
}

/// File and intermediate metadata belonging to a single detection run.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionArtifacts {
    /// Input document metadata, omitted if the document was deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<DetectionArtifactInput>,
    /// Intermediate artifact availability; no artifact content is embedded.
    pub intermediate: DetectionArtifactIntermediate,
    /// Live redacted output documents, newest first.
    pub redacted_outputs: Vec<DetectionArtifactOutput>,
}

/// The detection's input file.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionArtifactInput {
    /// Document identifier used by the document endpoints.
    pub id: Uuid,
    /// Display name.
    pub display_name: String,
}

/// Availability of the detection's intermediate artifact.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DetectionArtifactIntermediate {
    /// True while the intermediate's bytes are retained.
    pub available: bool,
}

/// A redacted file produced from the detection.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionArtifactOutput {
    /// The redaction pass that produced this file.
    pub redaction_id: nvisy_postgres::types::RedactionId,
    /// Document identifier used by the document endpoints.
    pub document_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// File extension without a dot.
    pub extension: String,
    /// Plaintext size in bytes.
    pub size: i64,
}

impl From<crate::domain::output::DetectionArtifacts> for WorkspaceDetectionArtifacts {
    fn from(artifacts: crate::domain::output::DetectionArtifacts) -> Self {
        Self {
            input: artifacts.input.map(|document| DetectionArtifactInput {
                id: document.id,
                display_name: document.display_name,
            }),
            intermediate: DetectionArtifactIntermediate {
                available: artifacts.intermediate_available,
            },
            redacted_outputs: artifacts
                .redacted_outputs
                .into_iter()
                .map(|output| DetectionArtifactOutput {
                    redaction_id: nvisy_postgres::types::RedactionId::from_uuid(
                        output.redaction_id,
                    ),
                    document_id: output.document_id,
                    display_name: output.display_name,
                    extension: output.extension,
                    size: output.size,
                })
                .collect(),
        }
    }
}
