//! Response types for OCR operations.
//!
//! The `Response<Resp>` type is a generic wrapper that allows OCR implementations
//! to define their own response payload types while maintaining a consistent
//! interface for common metadata like response IDs, timestamps, and usage statistics.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::context::UsageStats;

/// Generic response from an OCR operation.
///
/// This wrapper type provides common metadata and statistics while allowing
/// implementations to define their own specific response payload type.
///
/// # Type Parameters
///
/// * `Resp` - The implementation-specific response payload type
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone)]
/// struct MyOcrResponse {
///     text: String,
///     confidence: f32,
/// }
///
/// let response = Response::new(
///     request_id,
///     MyOcrResponse {
///         text: "extracted text".to_string(),
///         confidence: 0.95,
///     }
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<Resp> {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Implementation-specific response payload.
    pub payload: Resp,
    /// Processing time in milliseconds.
    pub processing_time_ms: Option<u64>,
    /// When this response was generated.
    pub timestamp: Timestamp,
    /// Usage statistics for this operation.
    pub usage: UsageStats,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<Resp> Response<Resp> {
    /// Create a new OCR response with the given payload.
    pub fn new(request_id: Uuid, payload: Resp) -> Self {
        Self {
            response_id: Uuid::new_v4(),
            request_id,
            payload,
            processing_time_ms: None,
            timestamp: Timestamp::now(),
            usage: UsageStats::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set the processing time.
    pub fn with_processing_time(mut self, ms: u64) -> Self {
        self.processing_time_ms = Some(ms);
        self
    }

    /// Set usage statistics.
    pub fn with_usage(mut self, usage: UsageStats) -> Self {
        self.usage = usage;
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Batch response containing multiple OCR results.
///
/// # Type Parameters
///
/// * `Resp` - The implementation-specific response payload type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse<Resp> {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<Response<Resp>>,
    /// Overall processing statistics.
    pub batch_stats: BatchStats,
    /// When the batch was processed.
    pub timestamp: Timestamp,
}

/// Statistics for a batch OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStats {
    /// Total number of items processed.
    pub total_processed: usize,
    /// Number of successful extractions.
    pub successful: usize,
    /// Number of failed extractions.
    pub failed: usize,
    /// Total processing time for the batch.
    pub total_processing_time_ms: u64,
    /// Average confidence across all successful extractions.
    pub average_confidence: f32,
}

impl BatchStats {
    /// Calculate success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        if self.total_processed == 0 {
            0.0
        } else {
            (self.successful as f32 / self.total_processed as f32) * 100.0
        }
    }

    /// Get average processing time per item.
    pub fn average_processing_time(&self) -> f32 {
        if self.total_processed == 0 {
            0.0
        } else {
            self.total_processing_time_ms as f32 / self.total_processed as f32
        }
    }
}
