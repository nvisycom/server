//! Detection service outputs.

use nvisy_postgres::model::WorkspaceDetection as WorkspaceDetectionModel;
use nvisy_postgres::query::DetectionDocuments;
use nvisy_postgres::types::Handle;
use uuid::Uuid;

/// A newly created or idempotently replayed detection, with the context a response
/// renders it from without follow-up lookups.
///
/// The triggering account is carried as an id; the handler resolves it to a public
/// reference when building the response.
pub struct CreatedDetection {
    /// The detection row.
    pub detection: WorkspaceDetectionModel,
    /// Slug of the detection's owning pipeline, `None` for an ad-hoc detection.
    pub pipeline_slug: Option<Handle>,
    /// The account that triggered the detection.
    pub trigger_account_id: Uuid,
    /// The detection's input document name(s).
    pub documents: DetectionDocuments,
    /// `true` when this call created the detection (`201`/`202`), `false` when an
    /// idempotency key replayed an existing one (`200`).
    pub created: bool,
}
