//! Pipeline run status enumeration indicating the execution state of a pipeline run.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the execution status of a pipeline run.
///
/// This enumeration corresponds to the `PIPELINE_RUN_STATUS` PostgreSQL enum and is used
/// to track the current state of a pipeline execution.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::PipelineRunStatus"]
pub enum PipelineRunStatus {
    /// Detection in progress
    #[db_rename = "running"]
    #[serde(rename = "running")]
    #[default]
    Running,

    /// Detection done; awaiting reviewer verification
    #[db_rename = "analyzed"]
    #[serde(rename = "analyzed")]
    Analyzed,

    /// Redaction applied; run finished
    #[db_rename = "completed"]
    #[serde(rename = "completed")]
    Completed,

    /// Run failed with error
    #[db_rename = "failed"]
    #[serde(rename = "failed")]
    Failed,

    /// Run was cancelled by user
    #[db_rename = "cancelled"]
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl PipelineRunStatus {
    /// Returns whether detection is in progress.
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, PipelineRunStatus::Running)
    }

    /// Returns whether detection is done and the run awaits verification.
    #[inline]
    pub fn is_analyzed(self) -> bool {
        matches!(self, PipelineRunStatus::Analyzed)
    }

    /// Returns whether the run completed successfully.
    #[inline]
    pub fn is_completed(self) -> bool {
        matches!(self, PipelineRunStatus::Completed)
    }

    /// Returns whether the run failed.
    #[inline]
    pub fn is_failed(self) -> bool {
        matches!(self, PipelineRunStatus::Failed)
    }

    /// Returns whether the run was cancelled.
    #[inline]
    pub fn is_cancelled(self) -> bool {
        matches!(self, PipelineRunStatus::Cancelled)
    }

    /// Returns whether the run is still active (running or awaiting review).
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            PipelineRunStatus::Running | PipelineRunStatus::Analyzed
        )
    }

    /// Returns whether the run has finished (completed, failed, or cancelled).
    #[inline]
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            PipelineRunStatus::Completed | PipelineRunStatus::Failed | PipelineRunStatus::Cancelled
        )
    }

    /// Returns whether the run finished with a terminal error state.
    #[inline]
    pub fn is_terminal_error(self) -> bool {
        matches!(self, PipelineRunStatus::Failed)
    }

    /// Returns whether the run can be retried.
    #[inline]
    pub fn is_retriable(self) -> bool {
        matches!(
            self,
            PipelineRunStatus::Failed | PipelineRunStatus::Cancelled
        )
    }
}
