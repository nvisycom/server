//! Pipeline reference join-table constraint violations.

use strum::EnumString;

/// Foreign-key violations on the pipeline → policy join table.
///
/// These fire when a pipeline references a policy id that does not exist in its
/// workspace, so they map to a client error rather than a 500.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspacePipelineReferenceConstraints {
    #[strum(serialize = "workspace_pipeline_policies_policy_fkey")]
    PolicyReference,
}
