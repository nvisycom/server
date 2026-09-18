//! Redaction request types.

use nvisy_postgres::types::RedactionFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Query parameters for listing a workspace's redactions.
///
/// Every field is an optional filter; unset fields impose no constraint. A
/// redaction has no status of its own, so it narrows by its owning detection and
/// the document that detection analyzed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRedactionsQuery {
    /// Filter to the redactions produced from a specific detection.
    pub detection_id: Option<Uuid>,
    /// Filter to the redactions of detections analyzing a specific document.
    pub document_id: Option<Uuid>,
}

impl From<WorkspaceRedactionsQuery> for RedactionFilter {
    fn from(query: WorkspaceRedactionsQuery) -> Self {
        RedactionFilter {
            detection_id: query.detection_id,
            document_id: query.document_id,
        }
    }
}
