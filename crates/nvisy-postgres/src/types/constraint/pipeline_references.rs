//! Pipeline reference join-table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Foreign-key violations on the pipeline → policy join table.
///
/// These fire when a pipeline references a policy id that does not exist in its
/// workspace, so they map to a client error rather than a 500.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineReferenceConstraints {
    // Foreign-key constraints (referenced row must exist in the workspace)
    #[strum(serialize = "workspace_pipeline_policies_policy_fkey")]
    PolicyReference,
    #[strum(serialize = "workspace_pipeline_policies_pipeline_fkey")]
    PolicyPipelineReference,
}

impl WorkspacePipelineReferenceConstraints {
    /// Creates a new [`WorkspacePipelineReferenceConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<WorkspacePipelineReferenceConstraints> for String {
    #[inline]
    fn from(val: WorkspacePipelineReferenceConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspacePipelineReferenceConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
