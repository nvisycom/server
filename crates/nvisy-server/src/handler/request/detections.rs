//! Detection request types (detect and redact).

use elide_pipeline::DocumentContext;
use elide_pipeline::entity::EditSet;
use nvisy_postgres::types::{DetectionFilter, DetectionStatus, PipelineTriggerType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Query parameters for listing detections across a workspace.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionsQuery {
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source file the detection analyzes.
    pub file_id: Option<Uuid>,
    /// Filter by the owning pipeline.
    pub pipeline_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    pub triggered_by: Option<Uuid>,
    /// Filter by how the detection was initiated (user vs system).
    pub trigger_type: Option<PipelineTriggerType>,
}

impl From<WorkspaceDetectionsQuery> for DetectionFilter {
    fn from(query: WorkspaceDetectionsQuery) -> Self {
        DetectionFilter {
            status: query.status,
            input_file_id: query.file_id,
            pipeline_id: query.pipeline_id,
            account_id: query.triggered_by,
            trigger_type: query.trigger_type,
        }
    }
}

/// Query parameters for listing a single pipeline's detections.
///
/// The pipeline is fixed by the route, so it narrows only by status, file,
/// trigger account, and trigger type.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDetectionsQuery {
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source file the detection analyzes.
    pub file_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    pub triggered_by: Option<Uuid>,
    /// Filter by how the detection was initiated (user vs system).
    pub trigger_type: Option<PipelineTriggerType>,
}

impl From<PipelineDetectionsQuery> for DetectionFilter {
    fn from(query: PipelineDetectionsQuery) -> Self {
        DetectionFilter {
            status: query.status,
            input_file_id: query.file_id,
            pipeline_id: None,
            account_id: query.triggered_by,
            trigger_type: query.trigger_type,
        }
    }
}

/// Request payload to start a detection over a file.
///
/// Analyzes the file with the pipeline's configuration and returns the
/// detection, which holds the findings for review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateDetection {
    /// The file to analyze.
    pub file_id: Uuid,
    /// Per-document scope (languages, jurisdictions, document labels).
    ///
    /// Overrides the pipeline's `defaultScope` when present; absent falls back to
    /// the pipeline default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
}

/// Request payload to redact a detection.
///
/// The reviewer's edits layer over the detection's analysis before redaction:
/// suppress a false positive, retag a detection, or add one the analysis missed.
/// Omit `edits` to redact with the policy decisions exactly as detected. Each
/// redact request produces a new redaction.
#[must_use]
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactDetection {
    /// Reviewer edits to apply before redaction, grouped by modality. Omit to
    /// redact with the policy decisions exactly as detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditSet>,
}
