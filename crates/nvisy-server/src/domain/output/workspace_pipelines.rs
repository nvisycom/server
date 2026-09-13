//! Pipeline service outputs.

use nvisy_postgres::model::WorkspacePipeline;
use nvisy_postgres::types::Handle;

/// A pipeline paired with the policy slugs it references.
///
/// The slugs are the current references, so the handler builds the response
/// without reading the join table back.
pub struct PipelineWithReferences {
    /// The pipeline row.
    pub pipeline: WorkspacePipeline,
    /// Slugs of the policies the pipeline references.
    pub policy_slugs: Vec<Handle>,
}
