//! Detection request types (detect and redact).

use elide_pipeline::entity::EditSet;
use elide_pipeline::provider::DocumentContext;
use garde::Validate;
use nvisy_postgres::types::{
    DetectionFilter, DetectionStatus, PipelineTriggerType, RetentionOverride,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::input::{CreateAdhocDetectionInput, CreateDetectionInput};

/// Query parameters for listing detections across a workspace.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionsQuery {
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source document the detection analyzes.
    pub document_id: Option<Uuid>,
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
            input_document_id: query.document_id,
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
pub struct WorkspacePipelineDetectionsQuery {
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source document the detection analyzes.
    pub document_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    pub triggered_by: Option<Uuid>,
    /// Filter by how the detection was initiated (user vs system).
    pub trigger_type: Option<PipelineTriggerType>,
}

impl From<WorkspacePipelineDetectionsQuery> for DetectionFilter {
    fn from(query: WorkspacePipelineDetectionsQuery) -> Self {
        DetectionFilter {
            status: query.status,
            input_document_id: query.document_id,
            pipeline_id: None,
            account_id: query.triggered_by,
            trigger_type: query.trigger_type,
        }
    }
}

/// Request payload to start a detection over a document.
///
/// Analyzes the document with the pipeline's configuration and returns the
/// detection, which holds the findings for review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceDetection {
    /// The document to analyze.
    pub document_id: Uuid,
    /// Per-document scope (languages, jurisdictions, document labels).
    ///
    /// Overrides the pipeline's `defaultScope` when present; absent falls back to
    /// the pipeline default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
}

impl From<CreateWorkspaceDetection> for CreateDetectionInput {
    fn from(request: CreateWorkspaceDetection) -> Self {
        CreateDetectionInput {
            document_id: request.document_id,
            scope: request.scope,
        }
    }
}

/// Request payload to start an ad-hoc detection over a document, naming its
/// policies directly rather than through a pipeline.
///
/// Analyzes the document against the given policies (authored or one-shot) and
/// returns the detection holding the findings for review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateAdhocWorkspaceDetection {
    /// The document to analyze.
    pub document_id: Uuid,
    /// The policies to run against, by id. At least one is required.
    #[garde(length(min = 1, max = 64))]
    pub policy_ids: Vec<Uuid>,
    /// Per-document scope (languages, jurisdictions, document labels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces. Absent falls
    /// back to the workspace retention baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_override: Option<RetentionOverride>,
}

impl From<CreateAdhocWorkspaceDetection> for CreateAdhocDetectionInput {
    fn from(request: CreateAdhocWorkspaceDetection) -> Self {
        CreateAdhocDetectionInput {
            document_id: request.document_id,
            policy_ids: request.policy_ids,
            scope: request.scope,
            retention_override: request.retention_override,
        }
    }
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
pub struct RedactWorkspaceDetection {
    /// Reviewer edits to apply before redaction, grouped by modality. Omit to
    /// redact with the policy decisions exactly as detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditSet>,
}
