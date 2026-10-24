//! Filtering options for detection queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::DetectionStatus;

/// Filter options for detections.
///
/// Each field narrows the result when set; unset fields impose no constraint. The
/// workspace scope is applied by the query itself, not carried here.
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
    /// Filter to detections that ran against a specific policy, matching any of
    /// its versions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Filter by the account that triggered the detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
}
