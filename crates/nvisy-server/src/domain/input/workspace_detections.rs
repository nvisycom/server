//! Detection service inputs.

use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::types::RetentionOverride;
use uuid::Uuid;

/// Input for starting a detection over a document against an explicit policy list.
pub struct CreateDetectionInput {
    /// The document to analyze.
    pub document_id: Uuid,
    /// The policies to run against, by id.
    pub policy_ids: Vec<Uuid>,
    /// Per-document scope for the run.
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces.
    pub retention_override: Option<RetentionOverride>,
}
