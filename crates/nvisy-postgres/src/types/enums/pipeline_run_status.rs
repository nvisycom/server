//! Pipeline run status enumeration indicating the execution state of a pipeline run.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the execution status of a pipeline run.
///
/// This enumeration corresponds to the `PIPELINE_RUN_STATUS` PostgreSQL enum and is used
/// to track the current state of a pipeline execution.
///
/// The detect phase has two states: `Queued` (the run is enqueued but no worker
/// has begun) and `Analyzing` (a worker is actively analyzing). They settle into
/// `Analyzed` (detection done, awaiting review), then `Completed` after redaction.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::PipelineRunStatus"]
pub enum PipelineRunStatus {
    /// Enqueued for detection; no worker has picked it up yet
    #[db_rename = "queued"]
    #[serde(rename = "queued")]
    #[default]
    Queued,

    /// A worker is actively analyzing the document
    #[db_rename = "analyzing"]
    #[serde(rename = "analyzing")]
    Analyzing,

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
    /// Statuses that carry a success/failure outcome: a run reached one of these
    /// iff it either completed redaction or failed. `Cancelled` is excluded — a
    /// cancelled run has no outcome — so this is the correct basis for an error
    /// rate (`failed / (completed + failed)`).
    pub const OUTCOMES: [PipelineRunStatus; 2] =
        [PipelineRunStatus::Completed, PipelineRunStatus::Failed];

    /// Returns whether detection is done and the run awaits verification.
    #[inline]
    pub fn is_analyzed(self) -> bool {
        matches!(self, PipelineRunStatus::Analyzed)
    }

    /// Returns whether detection is still pending (queued or analyzing).
    #[inline]
    pub fn is_detecting(self) -> bool {
        matches!(
            self,
            PipelineRunStatus::Queued | PipelineRunStatus::Analyzing
        )
    }
}
