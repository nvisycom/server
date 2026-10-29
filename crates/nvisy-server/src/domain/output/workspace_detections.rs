//! Detection service outputs.

use nvisy_postgres::model::WorkspaceDetection as WorkspaceDetectionModel;
use nvisy_postgres::query::DetectionDocuments;
use uuid::Uuid;

/// A newly created or idempotently replayed detection, with the context a response
/// renders it from without follow-up lookups.
///
/// The triggering account is carried as an id; the handler resolves it to a public
/// reference when building the response.
pub struct CreatedDetection {
    /// The detection row.
    pub detection: WorkspaceDetectionModel,
    /// The account that triggered the detection.
    pub trigger_account_id: Uuid,
    /// The detection's input document name(s).
    pub documents: DetectionDocuments,
    /// `true` when this call created the detection (`201`/`202`), `false` when an
    /// idempotency key replayed an existing one (`200`).
    pub created: bool,
}

/// Artifact metadata for one detection run.
pub struct DetectionArtifacts {
    /// Live input document, if it has not been deleted.
    pub input: Option<nvisy_postgres::model::WorkspaceDocument>,
    /// Whether an intermediate artifact is retained.
    pub intermediate_available: bool,
    /// Live redacted output files.
    pub redacted_outputs: Vec<nvisy_postgres::query::DetectionRedactedOutput>,
}
