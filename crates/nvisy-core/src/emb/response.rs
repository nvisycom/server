//! Response types for embedding operations.
//!
//! This module defines the response types returned from embedding generation,
//! including embedding data, usage statistics, and response metadata.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response from an embedding generation request.
///
/// This struct represents a complete embedding response containing the
/// generated embeddings, usage information, and metadata.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_core::emb::EmbeddingResponse;
///
/// // Process embedding response
/// let response: EmbeddingResponse = service.embed(request).await?;
///
/// for (i, embedding) in response.data.iter().enumerate() {
///     println!("Embedding {}: {} dimensions", i, embedding.embedding.len());
/// }
///
/// if let Some(usage) = &response.usage {
///     println!("Total tokens used: {}", usage.total_tokens);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Unique identifier for this response, matching the request ID.
    pub request_id: Uuid,

    /// The embedding data for each input.
    pub data: Vec<EmbeddingData>,

    /// The model used for generating embeddings.
    pub model: String,

    /// Usage statistics for this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<EmbeddingUsage>,

    /// Additional metadata about the response.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
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

    /// The object type (always "embedding").
    #[serde(default = "default_object_type")]
    pub object: String,

    /// Additional metadata for this specific embedding.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Usage statistics for embedding generation.
///
/// This struct provides information about resource consumption
/// during the embedding generation process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// Number of tokens in the input(s).
    pub prompt_tokens: u32,

    /// Total number of tokens used (usually same as prompt_tokens for embeddings).
    pub total_tokens: u32,

    /// Additional usage metrics specific to the provider.
    #[serde(flatten)]
    pub additional_metrics: HashMap<String, serde_json::Value>,
}

fn default_object_type() -> String {
    "embedding".to_string()
}

impl EmbeddingResponse {
    /// Creates a new embedding response.
    pub fn new(request_id: Uuid, data: Vec<EmbeddingData>, model: String) -> Self {
        Self {
            request_id,
            data,
            model,
            usage: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates a new response builder.
    pub fn builder() -> EmbeddingResponseBuilder {
        EmbeddingResponseBuilder::new()
    }

    /// Returns the number of embeddings in this response.
    pub fn embedding_count(&self) -> usize {
        self.data.len()
    }

    /// Returns the dimensionality of the embeddings.
    ///
    /// All embeddings in a response should have the same dimensionality.
    /// Returns `None` if there are no embeddings in the response.
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
            true // Empty response is considered consistent
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

    /// Adds metadata to the response.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Gets metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Validates the response structure.
    pub fn validate(&self) -> Result<(), String> {
        if self.data.is_empty() {
            return Err("Response must contain at least one embedding".to_string());
        }

        if self.model.is_empty() {
            return Err("Model must be specified in response".to_string());
        }

        if !self.has_consistent_dimensions() {
            return Err("All embeddings must have the same dimensionality".to_string());
        }

        // Validate indices are sequential and start from 0
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
            object: default_object_type(),
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

impl EmbeddingUsage {
    /// Creates a new usage statistics entry.
    pub fn new(prompt_tokens: u32, total_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            total_tokens,
            additional_metrics: HashMap::new(),
        }
    }

    /// Adds an additional usage metric.
    pub fn with_metric(mut self, key: String, value: serde_json::Value) -> Self {
        self.additional_metrics.insert(key, value);
        self
    }

    /// Gets an additional metric by key.
    pub fn get_metric(&self, key: &str) -> Option<&serde_json::Value> {
        self.additional_metrics.get(key)
    }
}

/// Builder for creating embedding responses.
///
/// This builder provides a fluent interface for constructing embedding responses
/// with proper validation and defaults.
#[derive(Debug, Clone)]
pub struct EmbeddingResponseBuilder {
    request_id: Option<Uuid>,
    data: Vec<EmbeddingData>,
    model: Option<String>,
    usage: Option<EmbeddingUsage>,
    metadata: HashMap<String, serde_json::Value>,
}

impl EmbeddingResponseBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            request_id: None,
            data: Vec::new(),
            model: None,
            usage: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the request ID.
    pub fn request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Sets the embedding data.
    pub fn data(mut self, data: Vec<EmbeddingData>) -> Self {
        self.data = data;
        self
    }

    /// Adds a single embedding to the response.
    pub fn add_embedding(mut self, embedding: Vec<f32>) -> Self {
        let index = self.data.len();
        self.data.push(EmbeddingData::new(embedding, index));
        self
    }

    /// Sets the model name.
    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Sets the usage statistics.
    pub fn usage(mut self, usage: EmbeddingUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Adds metadata to the response.
    pub fn metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Builds the embedding response.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or if validation fails.
    pub fn build(self) -> Result<EmbeddingResponse, String> {
        let response = EmbeddingResponse {
            request_id: self.request_id.unwrap_or_else(Uuid::new_v4),
            data: self.data,
            model: self.model.ok_or("Model must be specified")?,
            usage: self.usage,
            metadata: self.metadata,
        };

        response.validate()?;
        Ok(response)
    }
}

impl Default for EmbeddingResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
