//! Detection service inputs.

use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::types::{Handle, RetentionOverride};
use uuid::Uuid;

/// Input for starting a detection over a document through a pipeline.
pub struct CreateDetectionInput {
    /// The document to analyze.
    pub document_id: Uuid,
    /// Per-document scope, overriding the pipeline's default when present.
    pub scope: Option<DocumentContext>,
}

/// Input for starting an ad-hoc detection against an explicit policy list.
pub struct CreateAdhocDetectionInput {
    /// The document to analyze.
    pub document_id: Uuid,
    /// The policies to run against, by slug.
    pub policy_slugs: Vec<Handle>,
    /// Per-document scope for the run.
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces.
    pub retention_override: Option<RetentionOverride>,
}
