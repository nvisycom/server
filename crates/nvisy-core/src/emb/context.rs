//! Context management for embedding operations.
//!
//! This module provides context types for managing embedding processing sessions,
//! including input texts, generated embeddings, processing options, and quality metrics.
//!
//! The `Context` type serves as a stateful container that tracks the entire embedding
//! processing lifecycle, from text input through embedding generation to quality assessment.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::request::EncodingFormat;

/// Context information for embedding operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier for this context session.
    pub context_id: Uuid,
    /// User identifier associated with this context.
    pub user_id: Uuid,
    /// Document identifier for tracking related embeddings.
    pub document_id: Option<Uuid>,
    /// The embedding service provider name.
    pub provider: String,
    /// The model used for embeddings.
    pub model: String,
    /// Processing options and configuration.
    pub processing_options: ProcessingOptions,
    /// Generated embeddings from previous operations.
    pub embeddings: Vec<EmbeddingResult>,
    /// Processing quality metrics.
    pub quality_metrics: QualityMetrics,
    /// Usage statistics for this context.
    pub usage: UsageStats,
    /// Metadata about the context and processing.
    pub metadata: ContextMetadata,
}

impl Context {
    /// Create a new embedding context.
    pub fn new(user_id: Uuid, provider: String, model: String) -> Self {
        Self {
            context_id: Uuid::new_v4(),
            user_id,
            document_id: None,
            provider,
            model,
            processing_options: ProcessingOptions::default(),
            embeddings: Vec::new(),
            quality_metrics: QualityMetrics::default(),
            usage: UsageStats::default(),
            metadata: ContextMetadata::default(),
        }
    }

    /// Set document identifier.
    pub fn set_document_id(&mut self, document_id: Uuid) {
        self.document_id = Some(document_id);
    }

    /// Add embedding result to the context.
    pub fn add_embedding_result(&mut self, mut result: EmbeddingResult) {
        result.embedding_id = Uuid::new_v4();
        result.processed_at = Timestamp::now();

        // Update usage statistics
        self.usage.total_tokens += result.token_count;
        self.usage.total_inputs += 1;
        self.usage.total_embeddings += result.embeddings.len() as u32;
        self.usage.successful_requests += 1;

        // Update quality metrics
        self.quality_metrics.update_dimensions(result.dimensions);

        self.embeddings.push(result);
        self.metadata.last_updated = Timestamp::now();
    }

    /// Get all embedding vectors.
    pub fn get_all_embeddings(&self) -> Vec<&Vec<f32>> {
        self.embeddings
            .iter()
            .flat_map(|result| &result.embeddings)
            .collect()
    }

    /// Get embeddings for specific input texts.
    pub fn get_embeddings_for_texts(&self, texts: &[String]) -> Vec<&Vec<f32>> {
        self.embeddings
            .iter()
            .filter(|result| result.input_texts.iter().any(|input| texts.contains(input)))
            .flat_map(|result| &result.embeddings)
            .collect()
    }

    /// Get total number of embeddings generated.
    pub fn embeddings_count(&self) -> u32 {
        self.usage.total_embeddings
    }

    /// Get total tokens processed.
    pub fn tokens_processed(&self) -> u32 {
        self.usage.total_tokens
    }

    /// Check if context has any embeddings.
    pub fn has_embeddings(&self) -> bool {
        !self.embeddings.is_empty()
    }

    /// Clear all embeddings.
    pub fn clear_embeddings(&mut self) {
        self.embeddings.clear();
        self.usage = UsageStats::default();
        self.quality_metrics = QualityMetrics::default();
    }
}

/// Processing options for embedding operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOptions {
    /// Encoding format for embeddings.
    pub encoding_format: EncodingFormat,
    /// Maximum number of inputs per batch request.
    pub max_batch_size: usize,
    /// Custom dimensions (if supported by model).
    pub dimensions: Option<u32>,
    /// Whether to normalize embeddings.
    pub normalize: bool,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            encoding_format: EncodingFormat::Float,
            max_batch_size: 100,
            dimensions: None,
            normalize: false,
        }
    }
}

/// Result of embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// Unique identifier for this embedding result.
    pub embedding_id: Uuid,
    /// Input texts that were embedded.
    pub input_texts: Vec<String>,
    /// Generated embedding vectors.
    pub embeddings: Vec<Vec<f32>>,
    /// Number of dimensions in each embedding.
    pub dimensions: u32,
    /// Total tokens processed for this request.
    pub token_count: u32,
    /// Processing time in milliseconds.
    pub processing_time_ms: u32,
    /// Timestamp when this was processed.
    pub processed_at: Timestamp,
    /// Additional metadata for this result.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EmbeddingResult {
    /// Create new embedding result.
    pub fn new(input_texts: Vec<String>, embeddings: Vec<Vec<f32>>) -> Self {
        let dimensions = embeddings.first().map(|e| e.len() as u32).unwrap_or(0);

        Self {
            embedding_id: Uuid::new_v4(),
            input_texts,
            embeddings,
            dimensions,
            token_count: 0,
            processing_time_ms: 0,
            processed_at: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Set token count.
    pub fn with_token_count(mut self, token_count: u32) -> Self {
        self.token_count = token_count;
        self
    }

    /// Set processing time.
    pub fn with_processing_time(mut self, ms: u32) -> Self {
        self.processing_time_ms = ms;
        self
    }

    /// Add metadata entry.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if result is empty.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Get number of embeddings.
    pub fn count(&self) -> usize {
        self.embeddings.len()
    }
}

/// Quality metrics for embedding processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Dimensions from all embeddings.
    dimensions_seen: Vec<u32>,
    /// Number of failed requests.
    pub failed_requests: u32,
    /// Overall quality score.
    pub quality_score: f32,
    /// Consistency metrics.
    pub consistency_score: Option<f32>,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            dimensions_seen: Vec::new(),
            failed_requests: 0,
            quality_score: 1.0,
            consistency_score: None,
        }
    }
}

impl QualityMetrics {
    /// Update dimension tracking.
    pub fn update_dimensions(&mut self, dimensions: u32) {
        self.dimensions_seen.push(dimensions);
    }

    /// Get most common dimensions.
    pub fn common_dimensions(&self) -> Option<u32> {
        if self.dimensions_seen.is_empty() {
            None
        } else {
            // Find most frequent dimension
            let mut counts = HashMap::new();
            for &dim in &self.dimensions_seen {
                *counts.entry(dim).or_insert(0) += 1;
            }
            counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(dim, _)| dim)
        }
    }

    /// Check if dimensions are consistent.
    pub fn has_consistent_dimensions(&self) -> bool {
        if self.dimensions_seen.len() <= 1 {
            true
        } else {
            let first = self.dimensions_seen[0];
            self.dimensions_seen.iter().all(|&dim| dim == first)
        }
    }
}

/// Usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct UsageStats {
    /// Total tokens processed.
    pub total_tokens: u32,
    /// Total input texts processed.
    pub total_inputs: u32,
    /// Total embeddings generated.
    pub total_embeddings: u32,
    /// Total processing time in milliseconds.
    pub total_processing_time_ms: u32,
    /// Number of successful requests.
    pub successful_requests: u32,
    /// Number of failed requests.
    pub failed_requests: u32,
    /// Estimated cost for processing.
    pub estimated_cost: Option<f64>,
}

impl UsageStats {
    /// Get total number of requests (successful + failed).
    pub fn total_requests(&self) -> u32 {
        self.successful_requests + self.failed_requests
    }

    /// Calculate success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.successful_requests as f32 / total as f32) * 100.0
        }
    }

    /// Calculate average processing time per request.
    pub fn average_processing_time_per_request(&self) -> Option<f32> {
        if self.successful_requests == 0 {
            None
        } else {
            Some(self.total_processing_time_ms as f32 / self.successful_requests as f32)
        }
    }

    /// Calculate average tokens per input.
    pub fn average_tokens_per_input(&self) -> Option<f32> {
        if self.total_inputs == 0 {
            None
        } else {
            Some(self.total_tokens as f32 / self.total_inputs as f32)
        }
    }

    /// Check if there's any usage data.
    pub fn has_usage(&self) -> bool {
        self.total_requests() > 0
    }
}

/// Context metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// Context creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub last_updated: Timestamp,
    /// Embedding service version.
    pub service_version: Option<String>,
    /// Processing mode used.
    pub processing_mode: Option<String>,
    /// Custom tags for categorization.
    pub tags: Vec<String>,
}

impl Default for ContextMetadata {
    fn default() -> Self {
        let now = Timestamp::now();
        Self {
            created_at: now,
            last_updated: now,
            service_version: None,
            processing_mode: None,
            tags: Vec::new(),
        }
    }
}
