//! Annotation collection operations for QdrantClient.
//!
//! This module provides specialized functionality for managing annotation vectors,
//! including text annotations, image annotations, spatial data, and various
//! annotation types optimized for different use cases.

use std::future::Future;

use qdrant_client::qdrant::condition::ConditionOneOf;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::{FieldCondition, Filter, Match};

use crate::client::QdrantClient;
use crate::collection::SearchParams;
use crate::error::{Error, Result};
use crate::payload::annotation::{AnnotationPayload, AnnotationPoint, AnnotationType};
use crate::types::{Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Trait for annotation collection operations.
pub trait AnnotationCollection {
    /// Create the annotation collection with sensible defaults
    fn create_collection(&self, vector_size: u64) -> impl Future<Output = Result<()>> + Send;

    /// Delete the annotation collection
    fn delete_collection(&self) -> impl Future<Output = Result<()>> + Send;

    /// Insert a single annotation
    fn insert_annotation(
        &self,
        annotation: AnnotationPoint,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Insert multiple annotations in a batch
    fn insert_annotations(
        &self,
        annotations: Vec<AnnotationPoint>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Search annotations by vector
    fn search_annotations(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search for annotations by type
    fn search_annotations_by_type(
        &self,
        annotation_type: AnnotationType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Get a specific annotation by ID
    fn get_annotation(&self, id: PointId) -> impl Future<Output = Result<Option<Point>>> + Send;

    /// Delete annotation by ID
    fn delete_annotation(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Delete multiple annotations by IDs
    fn delete_annotations(&self, ids: Vec<PointId>) -> impl Future<Output = Result<()>> + Send;

    /// Delete a single point by ID
    fn delete_point(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Update annotation payload
    fn update_annotation_payload(
        &self,
        id: PointId,
        payload: AnnotationPayload,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Count annotations by type
    fn count_annotations_by_type(
        &self,
        annotation_type: AnnotationType,
    ) -> impl Future<Output = Result<u64>> + Send;
}

impl QdrantClient {
    const ANNOTATION_COLLECTION: &'static str = "annotations";
}

impl AnnotationCollection for QdrantClient {
    async fn create_collection(&self, vector_size: u64) -> Result<()> {
        use qdrant_client::qdrant::vectors_config::Config;
        use qdrant_client::qdrant::{CreateCollection, VectorsConfig};

        let vector_params = VectorParams::new(vector_size, Distance::Cosine);
        let create_collection = CreateCollection {
            collection_name: Self::ANNOTATION_COLLECTION.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(vector_params.to_qdrant_vector_params())),
            }),
            shard_number: None,
            replication_factor: None,
            write_consistency_factor: None,
            on_disk_payload: None,
            hnsw_config: None,
            wal_config: None,
            optimizers_config: None,
            timeout: None,
            metadata: std::collections::HashMap::new(),
            quantization_config: None,
            sharding_method: None,
            strict_mode_config: None,
            sparse_vectors_config: None,
        };

        self.create_collection(create_collection).await
    }

    async fn delete_collection(&self) -> Result<()> {
        self.delete_collection(Self::ANNOTATION_COLLECTION, None)
            .await
    }

    async fn insert_annotation(&self, annotation: AnnotationPoint) -> Result<()> {
        let point: Point = annotation.into();
        self.upsert_point(Self::ANNOTATION_COLLECTION, point, true)
            .await
    }

    async fn insert_annotations(&self, annotations: Vec<AnnotationPoint>) -> Result<()> {
        let points: Vec<Point> = annotations.into_iter().map(|a| a.into()).collect();
        self.upsert_points(Self::ANNOTATION_COLLECTION, points, true)
            .await
    }

    async fn search_annotations(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let request = qdrant_client::qdrant::SearchPoints {
            collection_name: Self::ANNOTATION_COLLECTION.to_string(),
            vector: query_vector.values,
            limit,
            with_vectors: Some(true.into()),
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            filter: None,
            score_threshold: search_params.score_threshold,
            offset: None,
            vector_name: None,
            params: None,
            read_consistency: None,
            shard_key_selector: None,
            timeout: None,
            sparse_indices: None,
        };

        let response = self
            .raw_client()
            .search_points(request)
            .await
            .map_err(|e| Error::connection().with_source(Box::new(e)))?;

        let results: Vec<SearchResult> = response
            .result
            .into_iter()
            .filter_map(|point| SearchResult::try_from(point).ok())
            .collect();

        // Validate annotation points
        let valid_results: Vec<SearchResult> = results
            .into_iter()
            .filter(|result| result.payload.contains_key("annotation_type"))
            .collect();

        Ok(valid_results)
    }

    async fn search_annotations_by_type(
        &self,
        annotation_type: AnnotationType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for annotation type
        let type_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "annotation_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(annotation_type.as_str().to_string())),
                    }),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                    datetime_range: None,
                    geo_polygon: None,
                    is_empty: None,
                    is_null: None,
                })),
            }],
            should: vec![],
            must_not: vec![],
            min_should: None,
        };

        match query_vector {
            Some(vector) => {
                // Vector search with type filter
                let request = qdrant_client::qdrant::SearchPoints {
                    collection_name: Self::ANNOTATION_COLLECTION.to_string(),
                    vector: vector.values,
                    limit,
                    with_vectors: Some(true.into()),
                    with_payload: Some(WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(true)),
                    }),
                    filter: Some(type_filter),
                    score_threshold: search_params.score_threshold,
                    offset: None,
                    vector_name: None,
                    params: None,
                    read_consistency: None,
                    shard_key_selector: None,
                    timeout: None,
                    sparse_indices: None,
                };

                let response = self
                    .raw_client()
                    .search_points(request)
                    .await
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results: Vec<SearchResult> = response
                    .result
                    .into_iter()
                    .filter_map(|point| SearchResult::try_from(point).ok())
                    .collect();

                Ok(results)
            }
            None => {
                // Filter-only search (no vector)
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: Self::ANNOTATION_COLLECTION.to_string(),
                    filter: Some(type_filter),
                    limit: Some(limit as u32),
                    with_vectors: Some(true.into()),
                    with_payload: Some(WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(true)),
                    }),
                    offset: None,
                    order_by: None,
                    read_consistency: None,
                    shard_key_selector: None,
                    timeout: None,
                };

                let response = self
                    .raw_client()
                    .scroll(request)
                    .await
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results: Vec<SearchResult> = response
                    .result
                    .into_iter()
                    .filter_map(|point| {
                        // Create a scored point for scroll results with score 1.0
                        let scored_point = qdrant_client::qdrant::ScoredPoint {
                            id: point.id,
                            payload: point.payload,
                            score: 1.0,
                            vectors: point.vectors,
                            shard_key: None,
                            order_value: None,
                            version: 0,
                        };
                        SearchResult::try_from(scored_point).ok()
                    })
                    .collect();

                Ok(results)
            }
        }
    }

    async fn get_annotation(&self, id: PointId) -> Result<Option<Point>> {
        self.get_point(Self::ANNOTATION_COLLECTION, id).await
    }

    async fn delete_annotation(&self, id: PointId) -> Result<()> {
        self.delete_points(Self::ANNOTATION_COLLECTION, vec![id], true)
            .await
    }

    async fn delete_annotations(&self, ids: Vec<PointId>) -> Result<()> {
        self.delete_points(Self::ANNOTATION_COLLECTION, ids, true)
            .await
    }

    async fn delete_point(&self, id: PointId) -> Result<()> {
        self.delete_point(Self::ANNOTATION_COLLECTION, id, true)
            .await
    }

    async fn update_annotation_payload(
        &self,
        id: PointId,
        payload: AnnotationPayload,
    ) -> Result<()> {
        // Get the existing point
        if let Some(mut point) = self
            .get_point(Self::ANNOTATION_COLLECTION, id.clone())
            .await?
        {
            // Update the payload with new annotation data
            let new_payload = payload.to_payload();
            point.payload.merge(&new_payload);

            // Update the updated_at timestamp
            let now = jiff::Timestamp::now().to_string();
            point.payload = point.payload.with("updated_at", now);

            // Upsert the updated point
            self.upsert_point(Self::ANNOTATION_COLLECTION, point, true)
                .await
        } else {
            Err(Error::not_found().with_message(format!("Annotation with ID {:?} not found", id)))
        }
    }

    async fn count_annotations_by_type(&self, annotation_type: AnnotationType) -> Result<u64> {
        // Create filter for annotation type
        let type_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "annotation_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(annotation_type.as_str().to_string())),
                    }),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                    datetime_range: None,
                    geo_polygon: None,
                    is_empty: None,
                    is_null: None,
                })),
            }],
            should: vec![],
            must_not: vec![],
            min_should: None,
        };

        let request = qdrant_client::qdrant::CountPoints {
            collection_name: Self::ANNOTATION_COLLECTION.to_string(),
            filter: Some(type_filter),
            exact: Some(false), // Use approximate count for better performance
            read_consistency: None,
            shard_key_selector: None,
            timeout: None,
        };

        let response = self
            .raw_client()
            .count(request)
            .await
            .map_err(|e| Error::connection().with_source(Box::new(e)))?;

        Ok(response.result.map(|r| r.count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_type_string_conversion() {
        assert_eq!(AnnotationType::Text.as_str(), "text");
        assert_eq!(AnnotationType::ImageRegion.as_str(), "image_region");
        assert_eq!(AnnotationType::Custom("test".to_string()).as_str(), "test");
    }
}
