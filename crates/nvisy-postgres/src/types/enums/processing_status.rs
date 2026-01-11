//! Processing status enumeration for document and file processing operations.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the current processing status of a file in the processing pipeline.
///
/// This enumeration corresponds to the `PROCESSING_STATUS` PostgreSQL enum and is used
/// to track the state of files as they progress through various processing stages
/// such as text extraction, OCR, transcription, and analysis.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
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

    /// Processing completed, file is ready for use
    #[db_rename = "ready"]
    #[serde(rename = "ready")]
    Ready,

    /// Processing was canceled by user or system
    #[db_rename = "canceled"]
    #[serde(rename = "canceled")]
    Canceled,
}

impl ProcessingStatus {
    /// Returns whether the file is in a state that allows processing.
    #[inline]
    pub fn can_be_processed(self) -> bool {
        matches!(self, ProcessingStatus::Pending)
    }

    /// Returns whether the file is currently being processed.
    #[inline]
    pub fn is_processing(self) -> bool {
        matches!(self, ProcessingStatus::Processing)
    }

    /// Returns whether the processing is in a final state.
    #[inline]
    pub fn is_final(self) -> bool {
        matches!(self, ProcessingStatus::Ready | ProcessingStatus::Canceled)
    }

    /// Returns whether the file is ready for use.
    #[inline]
    pub fn is_ready(self) -> bool {
        matches!(self, ProcessingStatus::Ready)
    }

    /// Returns whether the processing was canceled.
    #[inline]
    pub fn is_canceled(self) -> bool {
        matches!(self, ProcessingStatus::Canceled)
    }

    /// Returns whether the processing is pending (waiting to start).
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, ProcessingStatus::Pending)
    }

    /// Returns whether the processing can be retried.
    #[inline]
    pub fn can_be_retried(self) -> bool {
        matches!(self, ProcessingStatus::Ready | ProcessingStatus::Canceled)
    }

    /// Returns whether the processing can be canceled.
    #[inline]
    pub fn can_be_canceled(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Pending | ProcessingStatus::Processing
        )
    }

    /// Returns whether this status represents an active processing operation.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            ProcessingStatus::Pending | ProcessingStatus::Processing
        )
    }

    /// Returns processing statuses that are considered active (not final).
    pub fn active_statuses() -> &'static [ProcessingStatus] {
        &[ProcessingStatus::Pending, ProcessingStatus::Processing]
    }

    /// Returns processing statuses that represent final states.
    pub fn final_statuses() -> &'static [ProcessingStatus] {
        &[ProcessingStatus::Ready, ProcessingStatus::Canceled]
    }
}
