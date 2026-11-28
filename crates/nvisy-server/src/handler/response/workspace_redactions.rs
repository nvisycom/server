//! Redaction response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceRedaction as RedactionModel;
use nvisy_postgres::types::{DetectionId, RedactionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page, WorkspaceRef};

/// Response type for a redaction.
///
/// A redaction is one redact pass over a detection, produced with a specific set
/// of reviewer edits. It owns the redacted output document (downloadable through
/// the normal file endpoints) and a review audit recording what was redacted and
/// why (fetched from the redaction's `review` endpoint).
///
/// Named `WorkspaceRedactionResult` rather than `Redaction` because the engine's audit
/// schema already carries a `Redaction` (an audit event), and the two must not
/// collide in the generated `OpenAPI`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRedactionResult {
    /// Opaque identifier of the redaction.
    pub id: RedactionId,
    /// The detection this redaction was produced from.
    pub detection_id: DetectionId,
    /// The workspace it belongs to.
    #[serde(flatten)]
    pub workspace: WorkspaceRef,
    /// Redacted output document this redaction produced. `None` only if the file
    /// was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_document_id: Option<Uuid>,
    /// Account that requested the redaction.
    pub requested_by: AccountRef,
    /// When the redaction was created.
    pub created_at: Timestamp,
}

/// Paginated response for redactions.
pub type WorkspaceRedactionsPage = Page<WorkspaceRedactionResult>;

impl WorkspaceRedactionResult {
    /// Creates a redaction response from the database model, the owning workspace
    /// id and handle, and the requesting account.
    #[must_use]
    pub fn from_model(
        redaction: &RedactionModel,
        workspace: WorkspaceRef,
        requested_by: AccountRef,
    ) -> Self {
        Self {
            id: RedactionId::from_uuid(redaction.id),
            detection_id: DetectionId::from_uuid(redaction.detection_id),
            workspace,
            output_document_id: redaction.output_document_id,
            requested_by,
            created_at: redaction.created_at.into(),
        }
    }
}
