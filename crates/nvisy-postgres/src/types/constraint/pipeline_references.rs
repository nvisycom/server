//! Pipeline reference join-table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Foreign-key violations on the pipeline → policy / context join tables.
///
/// These fire when a pipeline references a policy or context id that does not
/// exist in its workspace, so they map to a client error rather than a 500.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum PipelineReferenceConstraints {
    // Foreign-key constraints (referenced row must exist in the workspace)
    #[strum(serialize = "workspace_pipeline_policies_policy_fkey")]
    PolicyReference,
    #[strum(serialize = "workspace_pipeline_contexts_context_fkey")]
    ContextReference,
    #[strum(serialize = "workspace_pipeline_policies_pipeline_fkey")]
    PolicyPipelineReference,
    #[strum(serialize = "workspace_pipeline_contexts_pipeline_fkey")]
    ContextPipelineReference,
}

impl PipelineReferenceConstraints {
    /// Creates a new [`PipelineReferenceConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        ConstraintCategory::BusinessLogic
    }
}

impl From<PipelineReferenceConstraints> for String {
    #[inline]
    fn from(val: PipelineReferenceConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for PipelineReferenceConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
