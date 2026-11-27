//! Detection service inputs.

use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::types::RetentionOverride;
use uuid::Uuid;

use crate::domain::input::DerivedPolicyInput;

/// What a detection runs against: exactly one source.
pub enum DetectionSourceInput {
    /// Workspace policies, by id (authored or derived).
    Policies {
        /// The policies to run against.
        policy_ids: Vec<Uuid>,
    },
    /// Built-in labels, run as a derived policy (an identical one is reused).
    Labels(DerivedPolicyInput),
}

/// Input for starting a detection over a document.
pub struct CreateDetectionInput {
    /// The document to analyze.
    pub document_id: Uuid,
    /// What the detection runs against.
    pub source: DetectionSourceInput,
    /// Per-document scope for the run.
    pub scope: Option<DocumentContext>,
    /// Retention override for the outputs this detection produces.
    pub retention_override: Option<RetentionOverride>,
}
