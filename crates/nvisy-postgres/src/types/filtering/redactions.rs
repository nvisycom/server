//! Filtering options for redaction queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Filter options for the workspace-wide redaction listing.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself (through the redaction's
/// detection), not carried here. Redactions have no status of their own, so the
/// narrowing dimensions are the source document and the owning detection.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RedactionFilter {
    /// Filter to the redactions produced from a specific detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_id: Option<Uuid>,
    /// Filter to the redactions of detections analyzing a specific document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<Uuid>,
}
