//! Domain-specific collection operations for nvisy platform.
//!
//! This module provides traits that extend QdrantClient with domain-specific
//! functionality for different types of collections:
//!
//! - **AnnotationOperations**: For storing and searching annotation vectors
//! - **ConversationOperations**: For managing conversation embeddings and context
//! - **DocumentOperations**: For document storage and semantic search
//!
//! Each trait provides specialized methods and configurations tailored
//! to their specific domain requirements.

pub mod annotation;
pub mod conversation;
pub mod document;

pub use annotation::AnnotationCollection;
pub use conversation::ConversationCollection;
pub use document::DocumentCollection;
use serde::{Deserialize, Serialize};

/// Common search parameters used across all collection types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchParams {
    /// Maximum number of results to return
    pub limit: Option<u64>,
    /// Score threshold - only return results with score >= threshold
    pub score_threshold: Option<f32>,
    /// Whether to include vectors in the search results
    pub with_vectors: bool,
    /// Whether to include payload in the search results
    pub with_payload: bool,
    /// Search parameters for HNSW algorithm
    pub hnsw_ef: Option<u64>,
    /// Whether to use exact search instead of approximate
    pub exact: bool,
}

impl SearchParams {
    /// Create default search parameters
    pub fn new() -> Self {
        Self {
            limit: Some(10),
            score_threshold: None,
            with_vectors: false,
            with_payload: true,
            hnsw_ef: None,
            exact: false,
        }
    }

    /// Set the result limit
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the score threshold
    pub fn score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }

    /// Include vectors in results
    pub fn with_vectors(mut self) -> Self {
        self.with_vectors = true;
        self
    }

    /// Include payload in results
    pub fn with_payload(mut self) -> Self {
        self.with_payload = true;
        self
    }

    /// Set HNSW ef parameter
    pub fn hnsw_ef(mut self, ef: u64) -> Self {
        self.hnsw_ef = Some(ef);
        self
    }

    /// Use exact search
    pub fn exact(mut self) -> Self {
        self.exact = true;
        self
    }
}

impl Default for SearchParams {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for working with collections.
pub mod utils {

    use crate::error::{Error, Result};
    use crate::types::{Distance, Vector, VectorParams};

    /// Create a standard vector configuration for text embeddings
    pub fn text_vector_config(dimensions: u64) -> VectorParams {
        VectorParams::new(dimensions, Distance::Cosine).on_disk(false) // Keep text vectors in memory for speed
    }

    /// Create a standard vector configuration for image embeddings
    pub fn image_vector_config(dimensions: u64) -> VectorParams {
        VectorParams::new(dimensions, Distance::Cosine).on_disk(true) // Images can be larger, store on disk
    }

    /// Create a standard vector configuration for multimodal embeddings
    pub fn multimodal_vector_config(dimensions: u64) -> VectorParams {
        VectorParams::new(dimensions, Distance::Cosine)
    }

    /// Validate that a vector has the expected dimensions
    pub fn validate_vector_dimensions(vector: &Vector, expected: usize) -> Result<()> {
        if vector.len() != expected {
            return Err(Error::invalid_input().with_message(format!(
                "Expected vector with {} dimensions, got {}",
                expected,
                vector.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Distance, Vector};

    #[test]
    fn test_search_params() {
        let params = SearchParams::new()
            .limit(20)
            .score_threshold(0.8)
            .with_vectors()
            .exact();

        assert_eq!(params.limit, Some(20));
        assert_eq!(params.score_threshold, Some(0.8));
        assert!(params.with_vectors);
        assert!(params.exact);
    }

    #[test]
    fn test_search_params_defaults() {
        let params = SearchParams::default();
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.score_threshold, None);
        assert!(!params.with_vectors);
        assert!(params.with_payload);
        assert_eq!(params.hnsw_ef, None);
        assert!(!params.exact);
    }

    #[test]
    fn test_vector_configs() {
        let text_config = utils::text_vector_config(384);
        assert_eq!(text_config.size, 384);
        assert_eq!(text_config.distance, Distance::Cosine);
        assert_eq!(text_config.on_disk, Some(false));

        let image_config = utils::image_vector_config(512);
        assert_eq!(image_config.size, 512);
        assert_eq!(image_config.on_disk, Some(true));
    }

    #[test]
    fn test_vector_validation() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);

        // Valid dimensions
        assert!(utils::validate_vector_dimensions(&vector, 3).is_ok());

        // Invalid dimensions
        assert!(utils::validate_vector_dimensions(&vector, 4).is_err());
    }
}
