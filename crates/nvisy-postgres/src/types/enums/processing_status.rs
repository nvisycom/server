//! Processing status enumeration for file processing pipeline tracking.
//! Processing status enumeration for document and file processing operations.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the current processing status of a file in the processing pipeline.
///
/// This enumeration corresponds to the `PROCESSING_STATUS` PostgreSQL enum and is used
/// to track the state of files as they progress through various processing stages
/// such as text extraction, OCR, transcription, and analysis.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::ProcessingStatus"]
pub enum ProcessingStatus {
    /// File is queued for processing and waiting to be picked up
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// File is currently being processed by the system
    #[db_rename = "processing"]
    #[serde(rename = "processing")]
    Processing,

    /// Processing completed successfully
    #[db_rename = "completed"]
    #[serde(rename = "completed")]
    Completed,

    /// Processing failed due to an error
    #[db_rename = "failed"]
    #[serde(rename = "failed")]
    Failed,

    /// Processing was canceled by user or system
    #[db_rename = "canceled"]
    #[serde(rename = "canceled")]
    Canceled,

    /// Processing was skipped (file doesn't require processing)
    #[db_rename = "skipped"]
    #[serde(rename = "skipped")]
    Skipped,

    /// File is queued for retry after a previous failure
    #[db_rename = "retry"]
    #[serde(rename = "retry")]
    Retry,
}

impl ProcessingStatus {
    /// Returns whether the file is in a state that allows processing.
    #[inline]
    pub fn can_be_processed(self) -> bool {
        matches!(self, ProcessingStatus::Pending | ProcessingStatus::Retry)
    }

    /// Returns whether the file is currently being processed.
    #[inline]
    pub fn is_processing(self) -> bool {
        matches!(self, ProcessingStatus::Processing)
    }

    /// Returns whether the processing is in a final state (completed or terminal).
    #[inline]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Completed
                | ProcessingStatus::Failed
                | ProcessingStatus::Canceled
                | ProcessingStatus::Skipped
        )
    }

    /// Returns whether the processing completed successfully.
    #[inline]
    pub fn is_successful(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Completed | ProcessingStatus::Skipped
        )
    }

    /// Returns whether the processing failed or was terminated.
    #[inline]
    pub fn is_failed(self) -> bool {
        matches!(self, ProcessingStatus::Failed | ProcessingStatus::Canceled)
    }

    /// Returns whether the processing is pending (waiting to start).
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, ProcessingStatus::Pending | ProcessingStatus::Retry)
    }

    /// Returns whether the file processing was skipped.
    #[inline]
    pub fn is_skipped(self) -> bool {
        matches!(self, ProcessingStatus::Skipped)
    }

    /// Returns whether the processing is queued for retry.
    #[inline]
    pub fn is_retry(self) -> bool {
        matches!(self, ProcessingStatus::Retry)
    }

    /// Returns whether the processing can be retried.
    #[inline]
    pub fn can_be_retried(self) -> bool {
        matches!(self, ProcessingStatus::Failed)
    }

    /// Returns whether the processing can be canceled.
    #[inline]
    pub fn can_be_canceled(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Pending | ProcessingStatus::Processing | ProcessingStatus::Retry
        )
    }

    /// Returns whether this status represents an active processing operation.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Pending | ProcessingStatus::Processing | ProcessingStatus::Retry
        )
    }

    /// Returns whether this status indicates the file needs attention.
    #[inline]
    pub fn needs_attention(self) -> bool {
        matches!(self, ProcessingStatus::Failed)
    }

    /// Returns processing statuses that are considered active (not final).
    pub fn active_statuses() -> &'static [ProcessingStatus] {
        &[
            ProcessingStatus::Pending,
            ProcessingStatus::Processing,
            ProcessingStatus::Retry,
        ]
    }

    /// Returns processing statuses that represent successful completion.
    pub fn successful_statuses() -> &'static [ProcessingStatus] {
        &[ProcessingStatus::Completed, ProcessingStatus::Skipped]
    }

    /// Returns processing statuses that represent failure or termination.
    pub fn failed_statuses() -> &'static [ProcessingStatus] {
        &[ProcessingStatus::Failed, ProcessingStatus::Canceled]
    }
}
