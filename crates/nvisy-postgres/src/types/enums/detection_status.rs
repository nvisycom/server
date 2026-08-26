//! Detection status enumeration indicating the execution state of a detection.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The execution status of a detection (one analysis pass of a file).
///
/// Corresponds to the `DETECTION_STATUS` PostgreSQL enum. A detection is
/// `Pending` (enqueued, no worker yet), then `Executing` (a worker is actively
/// analyzing), then settles into `Complete` (analysis done, ready to redact) or
/// `Failed`. Redaction is a separate, repeatable action over a complete
/// detection and does not change this status.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::DetectionStatus"]
pub enum DetectionStatus {
    /// Enqueued for detection; no worker has picked it up yet.
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// A worker is actively analyzing the document.
    #[db_rename = "executing"]
    #[serde(rename = "executing")]
    Executing,

    /// Analysis done; the detection is ready to redact.
    #[db_rename = "complete"]
    #[serde(rename = "complete")]
    Complete,

    /// Detection failed with an error.
    #[db_rename = "failed"]
    #[serde(rename = "failed")]
    Failed,
}

impl DetectionStatus {
    /// Statuses that carry a success/failure outcome: a detection reached one of
    /// these iff it either finished analysis or failed. This is the correct basis
    /// for an error rate (`failed / (complete + failed)`).
    pub const OUTCOMES: [DetectionStatus; 2] = [DetectionStatus::Complete, DetectionStatus::Failed];

    /// Returns whether analysis is done and the detection is ready to redact.
    #[inline]
    pub fn is_complete(self) -> bool {
        matches!(self, DetectionStatus::Complete)
    }

    /// Returns whether the detection has not finished analysis yet (pending or
    /// executing).
    #[inline]
    pub fn is_detecting(self) -> bool {
        matches!(self, DetectionStatus::Pending | DetectionStatus::Executing)
    }
}
