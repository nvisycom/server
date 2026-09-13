//! Pipeline service outputs.

use nvisy_postgres::model::WorkspacePipeline;
use uuid::Uuid;

/// A pipeline paired with the policy ids it references.
///
/// The ids are the current references, so the handler builds the response
/// without reading the join table back.
pub struct PipelineWithReferences {
    /// The pipeline row.
    pub pipeline: WorkspacePipeline,
    /// Ids of the policies the pipeline references.
    pub policy_ids: Vec<Uuid>,
}
