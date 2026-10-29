//! Detection request types (detect and redact).

use elide_pipeline::entity::EditSet;
use elide_pipeline::provider::DocumentContext;
use garde::Validate;
use nvisy_postgres::types::{DetectionFilter, DetectionStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::input::CreateDetectionInput;

/// Query parameters for listing detections across a workspace.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionsQuery {
    /// Case-insensitive substring search of the input display name or original filename.
    pub search: Option<String>,
    /// Filter by detection status.
    pub status: Option<DetectionStatus>,
    /// Filter by the source document the detection analyzes.
    pub document_id: Option<Uuid>,
    /// Filter to detections that ran against a specific policy (any version).
    pub policy_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    pub triggered_by: Option<Uuid>,
}

impl From<WorkspaceDetectionsQuery> for DetectionFilter {
    fn from(query: WorkspaceDetectionsQuery) -> Self {
        DetectionFilter {
            search: query.search,
            status: query.status,
            input_document_id: query.document_id,
            policy_id: query.policy_id,
            account_id: query.triggered_by,
        }
    }
}

/// Request payload to start a detection over a document, naming the policies to run
/// against.
///
/// Analyzes the document against the given policies (authored or derived) and returns
/// the detection, which holds the findings for review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceDetection {
    /// The document to analyze.
    pub document_id: Uuid,
    /// The policies to run against, by id. At least one is required.
    #[garde(length(min = 1, max = 128))]
    pub policy_ids: Vec<Uuid>,
    /// Per-document scope (languages, jurisdictions, document labels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces. Absent falls
    /// back to the workspace retention baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_override: Option<RetentionOverride>,
}

impl From<CreateWorkspaceDetection> for CreateDetectionInput {
    fn from(request: CreateWorkspaceDetection) -> Self {
        CreateDetectionInput {
            document_id: request.document_id,
            policy_ids: request.policy_ids,
            scope: request.scope,
            retention_override: request.retention_override,
        }
    }
}

/// Request payload to set a detection's accumulated reviewer edits.
///
/// The edits (suppress a false positive, retag a detection, add one the analysis
/// missed) persist on the detection and are layered over its base analysis at redact
/// time, so they survive between redactions. Omit `edits` to clear them.
#[must_use]
#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct EditWorkspaceDetection {
    /// The `editSetRev` the client last read for this detection. Required, so the
    /// guard is honest: the write is applied only if it still matches (a concurrent
    /// edit is not silently overwritten, and a caller cannot skip the check by
    /// omitting the field); a stale value is rejected with 409. A never-edited
    /// detection is at rev 0.
    pub edit_set_rev: i32,
    /// The reviewer edits to persist, grouped by modality. Omit to clear the
    /// detection's stored edits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditSet>,
}
