//! Search functionality and types for Qdrant operations.
//!
//! This module provides search functionality for querying Qdrant collections,
//! including vector similarity search, filtering, and result handling.

use serde::{Deserialize, Serialize};

use crate::error::QdrantError;
use crate::types::{Payload, Point, PointId, PointVectors, Vector};

/// Search parameters for configuring search behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchParams {
    /// Maximum number of results to return
    pub limit: Option<u64>,
    /// Offset for pagination
    pub offset: Option<u64>,
    /// Score threshold - only return results with score >= threshold
    pub score_threshold: Option<f32>,
    /// Whether to include vectors in the search results
    pub with_vectors: bool,
    /// Whether to include payload in the search results
    pub with_payload: bool,
    /// HNSW ef parameter for search accuracy vs speed
    pub hnsw_ef: Option<u64>,
    /// Whether to use exact search instead of approximate
    pub exact: bool,
}

impl SearchParams {
    /// Create default search parameters
    pub fn new() -> Self {
        Self {
            limit: Some(10),
            offset: None,
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

    /// Set the offset for pagination
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
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

/// A single search result from a Qdrant query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Point ID
    pub id: PointId,
    /// Similarity score
    pub score: f32,
    /// Vector data (if requested)
    pub vector: Option<Vector>,
    /// Payload data (if requested)
    pub payload: Payload,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(id: impl Into<PointId>, score: f32, payload: Payload) -> Self {
        Self {
            id: id.into(),
            score,
            vector: None,
            payload,
        }
    }

    /// Create a search result with vector
    pub fn with_vector(
        id: impl Into<PointId>,
        score: f32,
        vector: Vector,
        payload: Payload,
    ) -> Self {
        Self {
            id: id.into(),
            score,
            vector: Some(vector),
            payload,
        }
    }

    /// Create a search result from a point with default score
    pub fn from_point(point: Point, score: f32) -> Self {
        // Extract single vector if available
        let vector = match &point.vectors {
            PointVectors::Single(v) => Some(v.clone()),
            PointVectors::Named(vectors) => {
                // Take the first vector if multiple named vectors exist
                vectors.values().next().cloned()
            }
        };

        Self {
            id: point.id,
            score,
            vector,
            payload: point.payload,
        }
    }
}

impl TryFrom<qdrant_client::qdrant::ScoredPoint> for SearchResult {
    type Error = QdrantError;

    fn try_from(scored_point: qdrant_client::qdrant::ScoredPoint) -> Result<Self, Self::Error> {
        let id = match scored_point.id {
            Some(point_id) => PointId::from_qdrant_point_id(point_id)?,
            None => return Err(QdrantError::InvalidInput("Missing point ID".to_string())),
        };

        let payload = Payload::from_qdrant_payload(scored_point.payload);

        let vector = scored_point
            .vectors
            .map(|_v| {
                // Convert VectorsOutput to Vector - for now just return empty vector
                // This needs to be implemented based on the actual Qdrant client API
                Ok::<Vector, QdrantError>(Vector::new(vec![]))
            })
            .transpose()?;

        Ok(SearchResult {
            id,
            score: scored_point.score,
            vector,
            payload,
        })
    }
}

/// Collection of search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    /// The search results
    pub results: Vec<SearchResult>,
    /// The time taken for the search operation (in seconds)
    pub time_taken: Option<f32>,
}

impl SearchResults {
    /// Create new search results
    pub fn new(results: Vec<SearchResult>) -> Self {
        Self {
            results,
            time_taken: None,
        }
    }

    /// Create search results with timing information
    pub fn with_time(results: Vec<SearchResult>, time_taken: f32) -> Self {
        Self {
            results,
            time_taken: Some(time_taken),
        }
    }

    /// Get the number of results
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Check if results are empty
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get an iterator over the results
    pub fn iter(&self) -> std::slice::Iter<SearchResult> {
        self.results.iter()
    }

    /// Filter results by minimum score
    pub fn filter_by_score(self, min_score: f32) -> Self {
        let filtered_results = self
            .results
            .into_iter()
            .filter(|result| result.score >= min_score)
            .collect();

        Self {
            results: filtered_results,
            time_taken: self.time_taken,
        }
    }

    /// Sort results by score (descending)
    pub fn sort_by_score(mut self) -> Self {
        self.results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self
    }

    /// Take the first n results
    pub fn take(mut self, n: usize) -> Self {
        self.results.truncate(n);
        self
    }
}

impl From<Vec<SearchResult>> for SearchResults {
    fn from(results: Vec<SearchResult>) -> Self {
        Self::new(results)
    }
}

impl IntoIterator for SearchResults {
    type IntoIter = std::vec::IntoIter<SearchResult>;
    type Item = SearchResult;

    fn into_iter(self) -> Self::IntoIter {
        self.results.into_iter()
    }
}

impl<'a> IntoIterator for &'a SearchResults {
    type IntoIter = std::slice::Iter<'a, SearchResult>;
    type Item = &'a SearchResult;

    fn into_iter(self) -> Self::IntoIter {
        self.results.iter()
    }
}

impl std::ops::Index<usize> for SearchResults {
    type Output = SearchResult;

    fn index(&self, index: usize) -> &Self::Output {
        &self.results[index]
    }
}

impl std::ops::IndexMut<usize> for SearchResults {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.results[index]
    }
}

/// Batch search results for multiple queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchSearchResults {
    /// Results for each query
    pub batch_results: Vec<SearchResults>,
    /// The time taken for the entire batch operation (in seconds)
    pub time_taken: Option<f32>,
}

impl BatchSearchResults {
    /// Create new batch search results
    pub fn new(batch_results: Vec<SearchResults>) -> Self {
        Self {
            batch_results,
            time_taken: None,
        }
    }

    /// Create batch search results with timing information
    pub fn with_time(batch_results: Vec<SearchResults>, time_taken: f32) -> Self {
        Self {
            batch_results,
            time_taken: Some(time_taken),
        }
    }

    /// Get the number of queries in the batch
    pub fn len(&self) -> usize {
        self.batch_results.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.batch_results.is_empty()
    }

    /// Get results for a specific query index
    pub fn get(&self, index: usize) -> Option<&SearchResults> {
        self.batch_results.get(index)
    }

    /// Get an iterator over all query results
    pub fn iter(&self) -> std::slice::Iter<SearchResults> {
        self.batch_results.iter()
    }
}

impl From<Vec<SearchResults>> for BatchSearchResults {
    fn from(batch_results: Vec<SearchResults>) -> Self {
        Self::new(batch_results)
    }
}

impl IntoIterator for BatchSearchResults {
    type IntoIter = std::vec::IntoIter<SearchResults>;
    type Item = SearchResults;

    fn into_iter(self) -> Self::IntoIter {
        self.batch_results.into_iter()
    }
}

impl std::ops::Index<usize> for BatchSearchResults {
    type Output = SearchResults;

    fn index(&self, index: usize) -> &Self::Output {
        &self.batch_results[index]
    }
}

/// Batch search request for searching multiple queries at once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchSearchRequest {
    /// Collection name to search in
    pub collection_name: String,
    /// List of search vectors
    pub vectors: Vec<Vector>,
    /// Search parameters to apply to all searches
    pub params: SearchParams,
}

impl BatchSearchRequest {
    /// Create a new batch search request
    pub fn new(collection_name: String, vectors: Vec<Vector>) -> Self {
        Self {
            collection_name,
            vectors,
            params: SearchParams::default(),
        }
    }

    /// Add a search vector to the batch
    pub fn add_vector(mut self, vector: Vector) -> Self {
        self.vectors.push(vector);
        self
    }

    /// Set search parameters for all searches
    pub fn with_params(mut self, params: SearchParams) -> Self {
        self.params = params;
        self
    }

    /// Get the number of searches in the batch
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Builder for constructing search requests with fluent API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchBuilder {
    /// Collection name to search in
    pub collection_name: String,
    /// Search vector
    pub vector: Option<Vector>,
    /// Search parameters
    pub params: SearchParams,
}

impl SearchBuilder {
    /// Create a new search builder
    pub fn new(collection_name: String) -> Self {
        Self {
            collection_name,
            vector: None,
            params: SearchParams::default(),
        }
    }

    /// Set the search vector
    pub fn vector(mut self, vector: Vector) -> Self {
        self.vector = Some(vector);
        self
    }

    /// Set search parameters
    pub fn with_params(mut self, params: SearchParams) -> Self {
        self.params = params;
        self
    }

    /// Set the result limit
    pub fn limit(mut self, limit: u64) -> Self {
        self.params = self.params.limit(limit);
        self
    }

    /// Set the score threshold
    pub fn score_threshold(mut self, threshold: f32) -> Self {
        self.params = self.params.score_threshold(threshold);
        self
    }

    /// Include vectors in results
    pub fn with_vectors(mut self) -> Self {
        self.params = self.params.with_vectors();
        self
    }

    /// Include payload in results
    pub fn with_payload(mut self) -> Self {
        self.params = self.params.with_payload();
        self
    }

    /// Use exact search
    pub fn exact(mut self) -> Self {
        self.params = self.params.exact();
        self
    }

    /// Build the search request
    pub fn build(self) -> Result<SearchRequest, QdrantError> {
        let vector = self
            .vector
            .ok_or_else(|| QdrantError::InvalidInput("Search vector is required".to_string()))?;

        Ok(SearchRequest {
            collection_name: self.collection_name,
            vector,
            params: self.params,
        })
    }
}

/// A complete search request for querying a Qdrant collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Collection name to search in
    pub collection_name: String,
    /// Search vector
    pub vector: Vector,
    /// Search parameters
    pub params: SearchParams,
}

impl SearchRequest {
    /// Create a new search request
    pub fn new(collection_name: String, vector: Vector) -> Self {
        Self {
            collection_name,
            vector,
            params: SearchParams::default(),
        }
    }

    /// Create a search request with parameters
    pub fn with_params(collection_name: String, vector: Vector, params: SearchParams) -> Self {
        Self {
            collection_name,
            vector,
            params,
        }
    }

    /// Convert to a builder for further modification
    pub fn to_builder(self) -> SearchBuilder {
        SearchBuilder {
            collection_name: self.collection_name,
            vector: Some(self.vector),
            params: self.params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_search_result() {
        let result = SearchResult::new("test-id", 0.9, Payload::new());
        assert_eq!(result.score, 0.9);
        assert!(result.vector.is_none());
    }

    #[test]
    fn test_search_results() {
        let results = vec![
            SearchResult::new("id1", 0.9, Payload::new()),
            SearchResult::new("id2", 0.8, Payload::new()),
        ];

        let search_results = SearchResults::new(results);
        assert_eq!(search_results.len(), 2);
        assert!(!search_results.is_empty());
    }

    #[test]
    fn test_batch_search_request() {
        let vectors = vec![Vector::new(vec![1.0, 2.0]), Vector::new(vec![3.0, 4.0])];

        let batch_request = BatchSearchRequest::new("test-collection".to_string(), vectors);
        assert_eq!(batch_request.len(), 2);
        assert!(!batch_request.is_empty());
    }
}
