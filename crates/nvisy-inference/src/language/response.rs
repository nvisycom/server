//! VLM response types.

use std::collections::HashMap;

use derive_builder::Builder;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Timing;
use crate::service::UsageStats;

/// Response from a single VLM operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "VlmResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "VlmResponseError")
)]
pub struct VlmResponse {
    /// Unique identifier for this response.
    #[builder(default = "Uuid::now_v7()")]
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// The generated content.
    pub content: String,
    /// Timing information for the operation.
    #[builder(default)]
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    #[builder(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Usage statistics for this operation.
    #[builder(default)]
    pub usage: Option<UsageStats>,
}

/// Error type for VlmResponse builder.
pub type VlmResponseError = derive_builder::UninitializedFieldError;

impl VlmResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<VlmResponse, VlmResponseError> {
        self.build_inner()
    }

    /// Add metadata to this response.
    pub fn add_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

impl VlmResponse {
    /// Create a new VLM response with the given content.
    pub fn new(request_id: Uuid, content: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            content: content.into(),
            timing: None,
            metadata: HashMap::new(),
            usage: None,
        }
    }

    /// Create a builder for this response.
    pub fn builder() -> VlmResponseBuilder {
        VlmResponseBuilder::default()
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
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

    /// Get total tokens used.
    pub fn total_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.total_tokens)
    }
}

/// Batch response containing multiple VLM results.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "VlmBatchResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "VlmBatchResponseError")
)]
pub struct VlmBatchResponse {
    /// Unique identifier for this batch response.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    #[builder(default)]
    pub responses: Vec<VlmResponse>,
}

/// Error type for VlmBatchResponse builder.
pub type VlmBatchResponseError = derive_builder::UninitializedFieldError;

impl VlmBatchResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<VlmBatchResponse, VlmBatchResponseError> {
        self.build_inner()
    }

    /// Add a response to the batch.
    pub fn add_response(mut self, response: VlmResponse) -> Self {
        self.responses.get_or_insert_with(Vec::new).push(response);
        self
    }
}

impl VlmBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<VlmResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
    }

    /// Create a builder for this response.
    pub fn builder() -> VlmBatchResponseBuilder {
        VlmBatchResponseBuilder::default()
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
    fn test_vlm_response_builder() {
        let request_id = Uuid::new_v4();
        let response = VlmResponse::builder()
            .with_request_id(request_id)
            .with_content("Hello")
            .build()
            .unwrap();
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.content(), "Hello");
    }
}
