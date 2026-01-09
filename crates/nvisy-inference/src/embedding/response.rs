//! Embedding response types.

use std::collections::HashMap;
use std::num::NonZeroU32;

use derive_builder::Builder;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::UsageStats;
use crate::types::Timing;

/// Response from a single embedding operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "EmbeddingResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "EmbeddingResponseError")
)]
pub struct EmbeddingResponse {
    /// Unique identifier for this response.
    #[builder(default = "Uuid::now_v7()")]
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Generated embedding vector.
    pub embedding: Vec<f32>,
    /// Tokens processed for this request.
    #[builder(default)]
    pub tokens: Option<NonZeroU32>,
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

/// Error type for EmbeddingResponse builder.
pub type EmbeddingResponseError = derive_builder::UninitializedFieldError;

impl EmbeddingResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<EmbeddingResponse, EmbeddingResponseError> {
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

impl EmbeddingResponse {
    /// Create a new embedding response.
    pub fn new(request_id: Uuid, embedding: Vec<f32>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            embedding,
            tokens: None,
            timing: None,
            metadata: HashMap::new(),
            usage: None,
        }
    }

    /// Create a builder for this response.
    pub fn builder() -> EmbeddingResponseBuilder {
        EmbeddingResponseBuilder::default()
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Set the token count.
    pub fn with_tokens(mut self, count: NonZeroU32) -> Self {
        self.tokens = Some(count);
        self
    }

    /// Set the token count from a u32 value (ignores zero values).
    pub fn with_tokens_u32(mut self, count: u32) -> Self {
        self.tokens = NonZeroU32::new(count);
        self
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        self.timing.map(|t| t.duration())
    }

    /// Get the dimensionality of the embedding.
    pub fn dimensions(&self) -> usize {
        self.embedding.len()
    }
}

/// Batch response containing multiple embedding results.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "EmbeddingBatchResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "EmbeddingBatchResponseError")
)]
pub struct EmbeddingBatchResponse {
    /// Unique identifier for this batch response.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    #[builder(default)]
    pub responses: Vec<EmbeddingResponse>,
}

/// Error type for EmbeddingBatchResponse builder.
pub type EmbeddingBatchResponseError = derive_builder::UninitializedFieldError;

impl EmbeddingBatchResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<EmbeddingBatchResponse, EmbeddingBatchResponseError> {
        self.build_inner()
    }

    /// Add a response to the batch.
    pub fn add_response(mut self, response: EmbeddingResponse) -> Self {
        self.responses.get_or_insert_with(Vec::new).push(response);
        self
    }
}

impl EmbeddingBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<EmbeddingResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
    }

    /// Create a batch response from embeddings.
    pub fn from_embeddings(embeddings: Vec<Vec<f32>>) -> Self {
        let responses = embeddings
            .into_iter()
            .map(|embedding| EmbeddingResponse::new(Uuid::now_v7(), embedding))
            .collect();
        Self::new(responses)
    }

    /// Create a builder for this response.
    pub fn builder() -> EmbeddingBatchResponseBuilder {
        EmbeddingBatchResponseBuilder::default()
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

    /// Get all embeddings as a vector.
    pub fn embeddings(&self) -> Vec<&Vec<f32>> {
        self.responses.iter().map(|r| &r.embedding).collect()
    }

    /// Consume the batch and return all embeddings.
    pub fn into_embeddings(self) -> Vec<Vec<f32>> {
        self.responses.into_iter().map(|r| r.embedding).collect()
    }

    /// Get the total tokens processed across all responses.
    pub fn total_tokens(&self) -> u32 {
        self.responses
            .iter()
            .filter_map(|r| r.tokens)
            .map(|t| t.get())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_response_creation() {
        let request_id = Uuid::new_v4();
        let embedding = vec![0.1, 0.2, 0.3];
        let response = EmbeddingResponse::new(request_id, embedding);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.dimensions(), 3);
        assert!(response.timing.is_none());
    }

    #[test]
    fn test_embedding_response_builder() {
        let request_id = Uuid::new_v4();
        let response = EmbeddingResponse::builder()
            .with_request_id(request_id)
            .with_embedding(vec![0.1, 0.2, 0.3])
            .build()
            .unwrap();
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.dimensions(), 3);
    }

    #[test]
    fn test_embedding_batch_response() {
        let embeddings = vec![vec![0.1, 0.2], vec![0.3, 0.4], vec![0.5, 0.6]];
        let batch = EmbeddingBatchResponse::from_embeddings(embeddings);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch.embeddings().len(), 3);
    }
}
