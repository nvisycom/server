//! Filtering options for pipeline run queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{PipelineRunStatus, PipelineTriggerType};

/// Filter options for pipeline runs.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The owning pipeline (single-pipeline listing) and workspace scope are applied
/// by the query itself, not carried here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RunFilter {
    /// Filter by run status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PipelineRunStatus>,
    /// Filter by the source file the run analyzes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_id: Option<Uuid>,
    /// Filter by the owning pipeline. Ignored by the single-pipeline listing
    /// (already scoped to one pipeline); used by the workspace-wide listing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<Uuid>,
    /// Filter by the account that triggered the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    /// Filter by how the run was initiated (user vs system).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<PipelineTriggerType>,
}

impl RunFilter {
    /// Creates a new empty filter.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by run status.
    #[inline]
    pub fn with_status(mut self, status: PipelineRunStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Filters by the source file the run analyzes.
    #[inline]
    pub fn with_input_file_id(mut self, input_file_id: Uuid) -> Self {
        self.input_file_id = Some(input_file_id);
        self
    }

    /// Filters by the owning pipeline.
    #[inline]
    pub fn with_pipeline_id(mut self, pipeline_id: Uuid) -> Self {
        self.pipeline_id = Some(pipeline_id);
        self
    }

    /// Filters by the account that triggered the run.
    #[inline]
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Filters by how the run was initiated.
    #[inline]
    pub fn with_trigger_type(mut self, trigger_type: PipelineTriggerType) -> Self {
        self.trigger_type = Some(trigger_type);
        self
    }

    /// Returns whether any filter is active.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.input_file_id.is_none()
            && self.pipeline_id.is_none()
            && self.account_id.is_none()
            && self.trigger_type.is_none()
    }
}
