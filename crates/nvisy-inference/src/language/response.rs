//! VLM response types.

use std::collections::HashMap;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Timing;

/// Usage statistics for VLM operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlmUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens in the completion.
    pub completion_tokens: u32,
    /// Total number of tokens used.
    pub total_tokens: u32,
}

impl VlmUsage {
    /// Create a new usage record.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// Response from a single VLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// The generated content.
    pub content: String,
    /// Token usage information.
    pub usage: Option<VlmUsage>,
    /// Reason why generation finished.
    pub finish_reason: Option<String>,
    /// Confidence score for the response (0.0 to 1.0).
    pub confidence: Option<f64>,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VlmResponse {
    /// Create a new VLM response with the given content.
    pub fn new(request_id: Uuid, content: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            content: content.into(),
            usage: None,
            finish_reason: None,
            confidence: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new VLM response with content and usage.
    pub fn with_usage(request_id: Uuid, content: impl Into<String>, usage: VlmUsage) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            content: content.into(),
            usage: Some(usage),
            finish_reason: None,
            confidence: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Set the finish reason.
    pub fn with_finish_reason(mut self, reason: impl Into<String>) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the generated content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        self.timing.map(|t| t.duration())
    }

    /// Get the content length in characters.
    pub fn content_length(&self) -> usize {
        self.content.chars().count()
    }

    /// Check if the response is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Check if the response generation completed normally.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("complete") | Some("stop") | Some("end_turn") | None
        )
    }

    /// Check if the response was truncated due to length limits.
    pub fn is_truncated(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("length") | Some("max_tokens")
        )
    }

    /// Check if the response was stopped due to content filtering.
    pub fn is_filtered(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("content_filter") | Some("safety")
        )
    }

    /// Get total tokens used.
    pub fn total_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.total_tokens)
    }
}

/// Batch response containing multiple VLM results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmBatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<VlmResponse>,
}

impl VlmBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<VlmResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
    }

    /// Get the earliest start time from all responses.
    pub fn started_at(&self) -> Option<Timestamp> {
        self.responses
            .iter()
            .filter_map(|r| r.timing.map(|t| t.started_at))
            .min()
    }

    /// Get the latest end time from all responses.
    pub fn ended_at(&self) -> Option<Timestamp> {
        self.responses
            .iter()
            .filter_map(|r| r.timing.map(|t| t.ended_at))
            .max()
    }

    /// Get the total processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        match (self.started_at(), self.ended_at()) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    /// Get the number of responses.
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }

    /// Get all contents as a vector.
    pub fn contents(&self) -> Vec<&str> {
        self.responses.iter().map(|r| r.content()).collect()
    }

    /// Consume the batch and return all contents.
    pub fn into_contents(self) -> Vec<String> {
        self.responses.into_iter().map(|r| r.content).collect()
    }

    /// Get the total tokens used across all responses.
    pub fn total_tokens(&self) -> u32 {
        self.responses
            .iter()
            .filter_map(|r| r.usage.as_ref())
            .map(|u| u.total_tokens)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlm_response_creation() {
        let request_id = Uuid::new_v4();
        let content = "This is the response";
        let response = VlmResponse::new(request_id, content);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.content(), content);
        assert!(response.usage.is_none());
        assert!(response.timing.is_none());
    }

    #[test]
    fn test_vlm_usage() {
        let usage = VlmUsage::new(25, 75);
        assert_eq!(usage.prompt_tokens, 25);
        assert_eq!(usage.completion_tokens, 75);
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_vlm_response_status_methods() {
        let complete = VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("stop");
        assert!(complete.is_complete());
        assert!(!complete.is_truncated());
        assert!(!complete.is_filtered());

        let truncated = VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("length");
        assert!(!truncated.is_complete());
        assert!(truncated.is_truncated());

        let filtered =
            VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("content_filter");
        assert!(filtered.is_filtered());
    }
}
