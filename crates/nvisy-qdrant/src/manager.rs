//! Search management operations for Qdrant.

use std::sync::Arc;

use qdrant_client::qdrant::{Filter, SearchPoints};
use tracing::{debug, error, instrument, warn};

use crate::client::QdrantConnection;
use crate::error::{QdrantError, QdrantResult};
use crate::types::{IntoPointId, IntoVector, PointId};
use crate::{SearchResult, TRACING_TARGET_SEARCH};

/// Manager for Qdrant search operations.
#[derive(Clone)]
pub struct SearchManager {
    /// The underlying connection to Qdrant
    connection: Arc<QdrantConnection>,
}

impl SearchManager {
    /// Create a new search manager with the given connection.
    pub fn new(connection: Arc<QdrantConnection>) -> Self {
        Self { connection }
    }

    /// Perform a basic vector search in a collection.
    #[instrument(skip(self, vector), target = TRACING_TARGET_SEARCH)]
    pub async fn search<T: IntoVector>(
        &self,
        collection_name: &str,
        vector: T,
        limit: Option<u64>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = SearchParams::new().limit(limit.unwrap_or(10));
        self.search_with_params(collection_name, vector, search_params)
            .await
    }

    /// Perform a vector search with custom parameters.
    #[instrument(skip(self, vector), target = TRACING_TARGET_SEARCH)]
    pub async fn search_with_params<T: IntoVector>(
        &self,
        collection_name: &str,
        vector: T,
        params: SearchParams,
    ) -> QdrantResult<Vec<SearchResult>> {
        let vector = vector.into_vector();

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            vector_dim = vector.len(),
            limit = params.limit,
            score_threshold = params.score_threshold,
            "Performing vector search"
        );

        let request = SearchPoints {
            collection_name: collection_name.to_string(),
            vector,
            limit: params.limit,
            filter: params.filter,
            with_payload: Some(params.with_payload.into()),
            with_vectors: Some(params.with_vectors.into()),
            params: params.search_params,
            score_threshold: params.score_threshold,
            offset: params.offset,
            vector_name: params.vector_name,
            read_consistency: None,
            timeout: params.timeout,
            shard_key_selector: None,
            sparse_indices: None,
        };

        let response = self
            .connection
            .client()
            .search_points(request)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_SEARCH,
                    error = %e,
                    collection = collection_name,
                    "Vector search failed"
                );
                QdrantError::search_error(collection_name, e.to_string())
            })?;

        self.connection.record_success().await;

        let results: Vec<SearchResult> = response
            .result
            .into_iter()
            .map(|scored_point| SearchResult::try_from(scored_point))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| QdrantError::Conversion(e.to_string()))?;

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            result_count = results.len(),
            "Vector search completed successfully"
        );

        Ok(results)
    }

    /// Perform a vector search with payload filters.
    #[instrument(skip(self, vector, filter), target = TRACING_TARGET_SEARCH)]
    pub async fn search_with_filter<T: IntoVector>(
        &self,
        collection_name: &str,
        vector: T,
        filter: Filter,
        limit: Option<u64>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let params = SearchParams::new()
            .limit(limit.unwrap_or(10))
            .filter(filter);

        self.search_with_params(collection_name, vector, params)
            .await
    }

    /// Perform batch vector searches across multiple vectors.
    #[instrument(skip(self, vectors), target = TRACING_TARGET_SEARCH, fields(batch_size = vectors.len()))]
    pub async fn batch_search<T: IntoVector + Clone>(
        &self,
        collection_name: &str,
        vectors: Vec<T>,
        limit: Option<u64>,
    ) -> QdrantResult<Vec<Vec<SearchResult>>> {
        let batch_size = vectors.len();

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            batch_size = batch_size,
            "Starting batch search operation"
        );

        if vectors.is_empty() {
            return Ok(vec![]);
        }

        let mut results = Vec::with_capacity(batch_size);
        let mut errors = Vec::new();

        for (index, vector) in vectors.into_iter().enumerate() {
            match self.search(collection_name, vector, limit).await {
                Ok(search_results) => results.push(search_results),
                Err(e) => {
                    warn!(
                        target: TRACING_TARGET_SEARCH,
                        error = %e,
                        collection = collection_name,
                        batch_index = index,
                        "Search failed in batch operation"
                    );
                    errors.push(format!("Index {}: {}", index, e));
                    results.push(vec![]); // Empty results for failed search
                }
            }
        }

        if !errors.is_empty() {
            let error_count = errors.len();
            let success_count = batch_size - error_count;

            warn!(
                target: TRACING_TARGET_SEARCH,
                collection = collection_name,
                success_count = success_count,
                error_count = error_count,
                "Batch search completed with some failures"
            );

            // Return partial success if more than half succeeded
            if success_count > error_count {
                return Ok(results);
            } else {
                return Err(QdrantError::batch_operation_failed(error_count, batch_size));
            }
        }

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            batch_size = batch_size,
            "Batch search completed successfully"
        );

        Ok(results)
    }

    /// Search for similar points to a given point ID.
    #[instrument(skip(self), target = TRACING_TARGET_SEARCH)]
    pub async fn search_similar_to_point(
        &self,
        collection_name: &str,
        point_id: PointId,
        limit: Option<u64>,
    ) -> QdrantResult<Vec<SearchResult>> {
        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            point_id = %point_id,
            "Searching for similar points"
        );

        // First, get the point to extract its vector
        let point = self
            .connection
            .client()
            .get_points(qdrant_client::qdrant::GetPoints {
                collection_name: collection_name.to_string(),
                ids: vec![point_id.clone().into_point_id().into()],
                with_payload: Some(false.into()),
                with_vectors: Some(true.into()),
                read_consistency: None,
                shard_key_selector: None,
                timeout: None,
            })
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_SEARCH,
                    error = %e,
                    collection = collection_name,
                    point_id = %point_id,
                    "Failed to get reference point"
                );
                QdrantError::point_error("get_reference_point", e.to_string())
            })?;

        let reference_point =
            point.result.into_iter().next().ok_or_else(|| {
                QdrantError::point_not_found(collection_name, point_id.to_string())
            })?;

        // Extract vector from the point
        let vector = reference_point
            .vectors
            .and_then(|v| v.vectors_options)
            .and_then(|vo| match vo {
                qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(v) => Some(v.data),
                qdrant_client::qdrant::vectors_output::VectorsOptions::Vectors(vs) => {
                    // For named vectors, take the first one or a default one
                    vs.vectors.into_iter().next().map(|(_, v)| v.data)
                }
            })
            .ok_or_else(|| {
                QdrantError::InvalidInput("No vector found in reference point".to_string())
            })?;

        // Now search using the extracted vector, excluding the original point
        let mut search_params = SearchParams::new().limit(limit.unwrap_or(10) + 1); // +1 to account for self

        // Add filter to exclude the original point
        let exclude_filter = Filter {
            must_not: vec![qdrant_client::qdrant::Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::HasId(
                    qdrant_client::qdrant::HasIdCondition {
                        has_id: vec![point_id.clone().into_point_id().into()],
                    },
                )),
            }],
            ..Default::default()
        };

        search_params = search_params.filter(exclude_filter);

        let results = self
            .search_with_params(collection_name, vector, search_params)
            .await?;

        // Limit results to the requested amount (since we added +1 above)
        let final_results: Vec<SearchResult> = results
            .into_iter()
            .take(limit.unwrap_or(10) as usize)
            .collect();

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            point_id = %point_id,
            result_count = final_results.len(),
            "Similar points search completed"
        );

        Ok(final_results)
    }

    /// Perform a recommendation search (positive and negative examples).
    #[instrument(skip(self, positive, negative), target = TRACING_TARGET_SEARCH)]
    pub async fn recommend(
        &self,
        collection_name: &str,
        positive: Vec<PointId>,
        negative: Vec<PointId>,
        limit: Option<u64>,
    ) -> QdrantResult<Vec<SearchResult>> {
        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            positive_count = positive.len(),
            negative_count = negative.len(),
            "Performing recommendation search"
        );

        let request = qdrant_client::qdrant::RecommendPoints {
            collection_name: collection_name.to_string(),
            positive: positive
                .into_iter()
                .map(|id| id.into_point_id().into())
                .collect(),
            negative: negative
                .into_iter()
                .map(|id| id.into_point_id().into())
                .collect(),
            limit: limit.unwrap_or(10),
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            filter: None,
            params: None,
            score_threshold: None,
            offset: None,
            using: None,
            lookup_from: None,
            read_consistency: None,
            positive_vectors: vec![],
            negative_vectors: vec![],
            shard_key_selector: None,
            timeout: None,
            strategy: None,
        };

        let response = self
            .connection
            .client()
            .recommend(request)
            .await
            .map_err(|e| {
                error!(
                    target: TRACING_TARGET_SEARCH,
                    error = %e,
                    collection = collection_name,
                    "Recommendation search failed"
                );
                QdrantError::Connection(e)
            })?;

        self.connection.record_success().await;

        let results: Vec<SearchResult> = response
            .result
            .into_iter()
            .map(|scored_point| SearchResult::try_from(scored_point))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| QdrantError::Conversion(e.to_string()))?;

        debug!(
            target: TRACING_TARGET_SEARCH,
            collection = collection_name,
            result_count = results.len(),
            "Recommendation search completed"
        );

        Ok(results)
    }
}

/// Parameters for customizing search operations.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Maximum number of results to return
    pub limit: u64,
    /// Minimum score threshold
    pub score_threshold: Option<f32>,
    /// Whether to include payload in results
    pub with_payload: bool,
    /// Whether to include vectors in results
    pub with_vectors: bool,
    /// Filter conditions
    pub filter: Option<Filter>,
    /// Search parameters for the algorithm
    pub search_params: Option<qdrant_client::qdrant::SearchParams>,
    /// Offset for pagination
    pub offset: Option<u64>,
    /// Name of the vector to search (for named vectors)
    pub vector_name: Option<String>,
    /// Request timeout
    pub timeout: Option<u64>,
}

impl SearchParams {
    /// Create new search parameters with defaults.
    pub fn new() -> Self {
        Self {
            limit: 10,
            score_threshold: None,
            with_payload: true,
            with_vectors: false,
            filter: None,
            search_params: None,
            offset: None,
            vector_name: None,
            timeout: None,
        }
    }

    /// Set the maximum number of results.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }

    /// Set the minimum score threshold.
    pub fn score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }

    /// Include payload in the search results.
    pub fn with_payload(mut self, include: bool) -> Self {
        self.with_payload = include;
        self
    }

    /// Include vectors in the search results.
    pub fn with_vectors(mut self, include: bool) -> Self {
        self.with_vectors = include;
        self
    }

    /// Add filter conditions.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set custom search parameters.
    pub fn search_params(mut self, params: qdrant_client::qdrant::SearchParams) -> Self {
        self.search_params = Some(params);
        self
    }

    /// Set pagination offset.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set the vector name for named vectors.
    pub fn vector_name(mut self, name: String) -> Self {
        self.vector_name = Some(name);
        self
    }

    /// Set request timeout in seconds.
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl Default for SearchParams {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::QdrantConfig;

    #[test]
    fn test_search_params() {
        let params = SearchParams::new()
            .limit(20)
            .score_threshold(0.8)
            .with_vectors(true)
            .offset(10);

        assert_eq!(params.limit, 20);
        assert_eq!(params.score_threshold, Some(0.8));
        assert!(params.with_vectors);
        assert_eq!(params.offset, Some(10));
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_search_operations() {
        let qdrant_config = QdrantConfig::new("http://localhost:6334").unwrap();

        match QdrantConnection::new(qdrant_config).await {
            Ok(connection) => {
                let connection = Arc::new(connection);
                let search_manager = SearchManager::new(connection);

                let vector = vec![1.0, 2.0, 3.0, 4.0];

                // This would fail if collection doesn't exist, which is expected in tests
                let result = search_manager
                    .search("test_collection", vector, Some(5))
                    .await;

                // We don't assert success here since it depends on Qdrant being available
                // and having the right collections
                match result {
                    Ok(results) => {
                        println!("Search returned {} results", results.len());
                    }
                    Err(e) => {
                        println!("Expected error when collection doesn't exist: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Expected error when Qdrant not available: {}", e);
            }
        }
    }
}
