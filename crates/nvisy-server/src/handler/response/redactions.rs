//! Redaction response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceRedaction as RedactionModel;
use nvisy_postgres::types::{DetectionId, Handle, RedactionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Response type for a redaction.
///
/// A redaction is one redact pass over a detection, produced with a specific set
/// of reviewer edits. It owns the redacted output document (downloadable through
/// the normal file endpoints) and a review audit recording what was redacted and
/// why (fetched from the redaction's `review` endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Redaction {
    /// Opaque identifier of the redaction.
    pub id: RedactionId,
    /// The detection this redaction was produced from.
    pub detection_id: DetectionId,
    /// Handle of the workspace this redaction belongs to.
    pub workspace_slug: Handle,
    /// Redacted output document this redaction produced. `None` only if the file
    /// was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    /// Account that requested the redaction.
    pub requested_by: AccountRef,
    /// When the redaction was created.
    pub created_at: Timestamp,
}

/// Paginated response for redactions.
pub type RedactionsPage = Page<Redaction>;

impl Redaction {
    /// Creates a redaction response from the database model, the owning
    /// workspace slug, and the requesting account.
    pub fn from_model(
        redaction: RedactionModel,
        workspace_slug: Handle,
        requested_by: AccountRef,
    ) -> Self {
        Self {
            id: RedactionId::from_uuid(redaction.id),
            detection_id: DetectionId::from_uuid(redaction.detection_id),
            workspace_slug,
            output_file_id: redaction.output_file_id,
            requested_by,
            created_at: redaction.created_at.into(),
        }
    }
}
