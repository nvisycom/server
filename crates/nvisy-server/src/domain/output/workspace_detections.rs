//! Detection service outputs.

use nvisy_postgres::model::WorkspaceDetection as WorkspaceDetectionModel;
use nvisy_postgres::query::DetectionDocuments;
use nvisy_postgres::types::AccountRefRow;

/// A newly created or idempotently replayed detection, with the context a response
/// renders it from without follow-up lookups.
///
/// The triggering account is carried as an id; the handler resolves it to a public
/// reference when building the response.
pub struct CreatedDetection {
    /// The detection and its input document name(s).
    pub found: FoundDetection,
    /// `true` when this call created the detection (`201`/`202`), `false` when an
    /// idempotency key replayed an existing one (`200`).
    pub created: bool,
}

/// A detection with its input document name(s).
pub struct FoundDetection {
    /// The detection row.
    pub detection: WorkspaceDetectionModel,
    /// The account that triggered it.
    pub trigger: AccountRefRow,
    /// The detection's input document name(s).
    pub documents: DetectionDocuments,
}
