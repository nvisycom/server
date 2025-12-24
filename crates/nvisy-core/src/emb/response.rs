//! Response types for embedding operations.
//!
//! The `Response<Resp>` type is a generic wrapper that allows embedding implementations
//! to define their own response payload types while maintaining a consistent
//! interface for common metadata like response IDs, timestamps, and usage statistics.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::context::UsageStats;

/// Generic response from an embedding operation.
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
/// struct MyEmbeddingPayload {
///     embeddings: Vec<Vec<f32>>,
/// }
///
/// let response = Response::new(
///     request_id,
///     MyEmbeddingPayload {
///         embeddings: vec![vec![0.1, 0.2, 0.3]],
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
    /// Create a new embedding response with the given payload.
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

/// Batch response containing multiple embedding results.
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

/// Statistics for a batch embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStats {
    /// Total number of items processed.
    pub total_processed: usize,
    /// Number of successful generations.
    pub successful: usize,
    /// Number of failed generations.
    pub failed: usize,
    /// Total processing time for the batch.
    pub total_processing_time_ms: u64,
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

/// Standard embedding result containing embedding vectors.
///
/// This provides a standardized way to represent embedding results,
/// making it easier to integrate with downstream processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// The embedding data for each input.
    pub data: Vec<EmbeddingData>,
}

/// Individual embedding data for a single input.
///
/// This struct represents the embedding vector and associated metadata
/// for a single input item in the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingData {
    /// The embedding vector.
    pub embedding: Vec<f32>,

    /// The index of this embedding in the original request.
    pub index: usize,

    /// Additional metadata for this specific embedding.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EmbeddingResult {
    /// Creates a new embedding result.
    pub fn new(data: Vec<EmbeddingData>) -> Self {
        Self { data }
    }

    /// Returns the number of embeddings in this result.
    pub fn embedding_count(&self) -> usize {
        self.data.len()
    }

    /// Returns the dimensionality of the embeddings.
    ///
    /// All embeddings in a result should have the same dimensionality.
    /// Returns `None` if there are no embeddings.
    pub fn embedding_dimensions(&self) -> Option<usize> {
        self.data.first().map(|embedding| embedding.embedding.len())
    }

    /// Returns true if all embeddings have the same dimensionality.
    pub fn has_consistent_dimensions(&self) -> bool {
        if let Some(expected_dim) = self.embedding_dimensions() {
            self.data
                .iter()
                .all(|embedding| embedding.embedding.len() == expected_dim)
        } else {
            true
        }
    }

    /// Gets the embedding at the specified index.
    pub fn get_embedding(&self, index: usize) -> Option<&EmbeddingData> {
        self.data.get(index)
    }

    /// Gets all embedding vectors as a slice of slices.
    pub fn embeddings(&self) -> Vec<&[f32]> {
        self.data
            .iter()
            .map(|data| data.embedding.as_slice())
            .collect()
    }

    /// Validates the result structure.
    pub fn validate(&self) -> Result<(), String> {
        if self.data.is_empty() {
            return Err("Result must contain at least one embedding".to_string());
        }

        if !self.has_consistent_dimensions() {
            return Err("All embeddings must have the same dimensionality".to_string());
        }

        for (expected_index, embedding_data) in self.data.iter().enumerate() {
            if embedding_data.index != expected_index {
                return Err(format!(
                    "Embedding index mismatch: expected {}, got {}",
                    expected_index, embedding_data.index
                ));
            }

            if embedding_data.embedding.is_empty() {
                return Err(format!("Embedding {} cannot be empty", expected_index));
            }
        }

        Ok(())
    }
}

impl EmbeddingData {
    /// Creates a new embedding data entry.
    pub fn new(embedding: Vec<f32>, index: usize) -> Self {
        Self {
            embedding,
            index,
            metadata: HashMap::new(),
        }
    }

    /// Returns the dimensionality of this embedding.
    pub fn dimensions(&self) -> usize {
        self.embedding.len()
    }

    /// Adds metadata to this embedding data.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Gets metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Normalizes the embedding vector to unit length.
    ///
    /// This modifies the embedding in-place and returns the original magnitude.
    pub fn normalize(&mut self) -> f32 {
        let magnitude = self.magnitude();
        if magnitude > 0.0 {
            for value in &mut self.embedding {
                *value /= magnitude;
            }
        }
        magnitude
    }

    /// Returns the magnitude (L2 norm) of the embedding vector.
    pub fn magnitude(&self) -> f32 {
        self.embedding.iter().map(|&x| x * x).sum::<f32>().sqrt()
    }

    /// Computes the cosine similarity with another embedding.
    ///
    /// Returns `None` if the embeddings have different dimensions or if either
    /// embedding has zero magnitude.
    pub fn cosine_similarity(&self, other: &EmbeddingData) -> Option<f32> {
        if self.embedding.len() != other.embedding.len() {
            return None;
        }

        let dot_product: f32 = self
            .embedding
            .iter()
            .zip(other.embedding.iter())
            .map(|(&a, &b)| a * b)
            .sum();

        let magnitude_product = self.magnitude() * other.magnitude();
        if magnitude_product > 0.0 {
            Some(dot_product / magnitude_product)
        } else {
            None
        }
    }

    /// Computes the euclidean distance to another embedding.
    ///
    /// Returns `None` if the embeddings have different dimensions.
    pub fn euclidean_distance(&self, other: &EmbeddingData) -> Option<f32> {
        if self.embedding.len() != other.embedding.len() {
            return None;
        }

        let distance_squared: f32 = self
            .embedding
            .iter()
            .zip(other.embedding.iter())
            .map(|(&a, &b)| (a - b).powi(2))
            .sum();

        Some(distance_squared.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_result_creation() {
        let data = vec![
            EmbeddingData::new(vec![0.1, 0.2, 0.3], 0),
            EmbeddingData::new(vec![0.4, 0.5, 0.6], 1),
        ];

        let result = EmbeddingResult::new(data);

        assert_eq!(result.embedding_count(), 2);
        assert_eq!(result.embedding_dimensions(), Some(3));
        assert!(result.has_consistent_dimensions());
    }

    #[test]
    fn test_embedding_data_operations() {
        let embedding1 = EmbeddingData::new(vec![1.0, 0.0, 0.0], 0);
        let embedding2 = EmbeddingData::new(vec![0.0, 1.0, 0.0], 1);

        assert_eq!(embedding1.dimensions(), 3);
        assert!((embedding1.magnitude() - 1.0).abs() < f32::EPSILON);

        let similarity = embedding1.cosine_similarity(&embedding2);
        assert!(similarity.is_some());
        assert!((similarity.unwrap() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_embedding_normalization() {
        let mut embedding = EmbeddingData::new(vec![3.0, 4.0], 0);

        let magnitude = embedding.normalize();
        assert!((magnitude - 5.0).abs() < f32::EPSILON);
        assert!((embedding.magnitude() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_euclidean_distance() {
        let embedding1 = EmbeddingData::new(vec![0.0, 0.0], 0);
        let embedding2 = EmbeddingData::new(vec![3.0, 4.0], 1);

        let distance = embedding1.euclidean_distance(&embedding2);
        assert!(distance.is_some());
        assert!((distance.unwrap() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_result_validation() {
        let valid_result = EmbeddingResult::new(vec![
            EmbeddingData::new(vec![0.1, 0.2], 0),
            EmbeddingData::new(vec![0.3, 0.4], 1),
        ]);
        assert!(valid_result.validate().is_ok());

        let empty_result = EmbeddingResult::new(vec![]);
        assert!(empty_result.validate().is_err());

        let inconsistent_result = EmbeddingResult::new(vec![
            EmbeddingData::new(vec![0.1, 0.2], 0),
            EmbeddingData::new(vec![0.3, 0.4, 0.5], 1),
        ]);
        assert!(inconsistent_result.validate().is_err());
    }

    #[test]
    fn test_batch_stats() {
        let stats = BatchStats {
            total_processed: 10,
            successful: 8,
            failed: 2,
            total_processing_time_ms: 5000,
        };

        assert_eq!(stats.success_rate(), 80.0);
        assert_eq!(stats.average_processing_time(), 500.0);
    }
}
