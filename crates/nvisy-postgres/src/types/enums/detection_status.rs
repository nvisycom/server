//! Detection status enumeration indicating the execution state of a detection.

use super::db_enum;

db_enum! {
    /// The execution status of a detection (one analysis pass of a file).
    ///
    /// Corresponds to the `DETECTION_STATUS` PostgreSQL enum. A detection is
    /// `Pending` (enqueued, no worker yet), then `Executing` (a worker is actively
    /// analyzing), then settles into `Complete` (analysis done, ready to redact)
    /// or `Failed`. Redaction is a separate, repeatable action over a complete
    /// detection and does not change this status.
    pub enum DetectionStatus: Default = Pending, "crate::schema::sql_types::DetectionStatus" {
        /// Enqueued for detection; no worker has picked it up yet.
        Pending = "pending",
        /// A worker is actively analyzing the document.
        Executing = "executing",
        /// Analysis done; the detection is ready to redact.
        Complete = "complete",
        /// Detection failed with an error.
        Failed = "failed",
    }
}

impl DetectionStatus {
    /// In-progress statuses: a detection is still analyzing — enqueued or
    /// running — so its input and audit files must not expire yet. `Complete` is
    /// excluded (it is terminal), so holding it would pin those files forever.
    pub const IN_PROGRESS: [DetectionStatus; 2] =
        [DetectionStatus::Pending, DetectionStatus::Executing];
    /// Terminal statuses: a detection reached one of these iff it either finished
    /// analysis or failed, and its status will not change again. This is the
    /// correct basis for an error rate (`failed / (complete + failed)`).
    pub const TERMINAL: [DetectionStatus; 2] = [DetectionStatus::Complete, DetectionStatus::Failed];

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

    /// Monotonic rank of this status along the detection lifecycle
    /// (`pending` -> `executing` -> terminal).
    ///
    /// Status only ever moves forward, so comparing ranks lets a consumer drop an
    /// out-of-order update (e.g. a late `pending` after `executing` from a
    /// best-effort broadcast) rather than move backwards. Both terminal states
    /// share the top rank: a detection reaches exactly one, so they never need
    /// ordering against each other.
    #[inline]
    pub fn phase(self) -> u8 {
        match self {
            DetectionStatus::Pending => 0,
            DetectionStatus::Executing => 1,
            DetectionStatus::Complete | DetectionStatus::Failed => 2,
        }
    }
}
