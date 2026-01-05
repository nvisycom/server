//! Embedding response types.

use std::collections::HashMap;
use std::num::NonZeroU32;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Timing;

/// Format for returned embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingFormat {
    /// Return embeddings as floating point numbers.
    #[default]
    Float,
    /// Return embeddings as base64-encoded strings.
    Base64,
}

/// Response from a single embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Generated embedding vector.
    pub embedding: Vec<f32>,
    /// Tokens processed for this request.
    pub tokens: Option<NonZeroU32>,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
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
        }
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

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingBatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<EmbeddingResponse>,
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
    fn test_embedding_batch_response() {
        let embeddings = vec![vec![0.1, 0.2], vec![0.3, 0.4], vec![0.5, 0.6]];
        let batch = EmbeddingBatchResponse::from_embeddings(embeddings);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch.embeddings().len(), 3);
    }
}
