//! Filtering options for detection queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{DetectionStatus, PipelineTriggerType};

/// Filter options for detections.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The owning pipeline (single-pipeline listing) and workspace scope are applied
/// by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetectionFilter {
    /// Filter by detection status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DetectionStatus>,
    /// Filter by the source document the detection analyzes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_document_id: Option<Uuid>,
    /// Filter by the owning pipeline. Ignored by the single-pipeline listing
    /// (already scoped to one pipeline); used by the workspace-wide listing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    /// Filter by how the detection was initiated (user vs system).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<PipelineTriggerType>,
}
