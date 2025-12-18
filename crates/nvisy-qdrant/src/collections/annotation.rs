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
use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

use crate::client::QdrantClient;
use crate::collections::SearchParams;
use crate::error::{QdrantError, QdrantResult};
use crate::payload::{AnnotationPoint, AnnotationType};
use crate::types::{CollectionConfig, Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Configuration for annotation collections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct AnnotationConfig {
    /// Vector parameters for the collection
    pub vector_params: VectorParams,
    /// Whether to optimize for text annotations
    pub text_optimized: bool,
    /// Whether to optimize for image annotations
    pub image_optimized: bool,
    /// Enable spatial indexing for geometric annotations
    pub spatial_indexing: bool,
}

impl AnnotationConfig {
    /// Create a new annotation configuration
    pub fn new(dimensions: u64, distance: Distance) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, distance),
            text_optimized: false,
            image_optimized: false,
            spatial_indexing: false,
        }
    }

    /// Create configuration optimized for text annotations
    pub fn for_text_annotations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(false),
            text_optimized: true,
            image_optimized: false,
            spatial_indexing: false,
        }
    }

    /// Create configuration optimized for image annotations
    pub fn for_image_annotations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(true),
            text_optimized: false,
            image_optimized: true,
            spatial_indexing: false,
        }
    }

    /// Create configuration optimized for spatial/geometric annotations
    pub fn for_spatial_annotations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Euclid),
            text_optimized: false,
            image_optimized: false,
            spatial_indexing: true,
        }
    }

    /// Enable spatial indexing
    pub fn with_spatial_indexing(mut self) -> Self {
        self.spatial_indexing = true;
        self
    }

    /// Set vector parameters
    pub fn with_vectors(mut self, vector_params: VectorParams) -> Self {
        self.vector_params = vector_params;
        self
    }
}

/// Annotation operations trait for QdrantClient.
pub trait AnnotationOperations {
    /// Default collection name for annotations
    const DEFAULT_COLLECTION: &'static str = "annotations";

    /// Create an annotation collection
    fn create_annotation_collection(
        &self,
        name: &str,
        config: AnnotationConfig,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Insert a single annotation
    fn insert_annotation(
        &self,
        collection_name: &str,
        annotation: AnnotationPoint,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Insert multiple annotations
    /// Insert multiple annotations in a batch
    fn insert_annotations(
        &self,
        collection_name: &str,
        annotations: Vec<AnnotationPoint>,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Search annotations by vector
    fn search_annotations(
        &self,
        collection_name: &str,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> impl Future<Output = QdrantResult<Vec<SearchResult>>> + Send;

    /// Search for annotations by type
    fn search_annotations_by_type(
        &self,
        collection_name: &str,
        annotation_type: AnnotationType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = QdrantResult<Vec<SearchResult>>> + Send;

    /// Get annotation by ID
    /// Get a specific annotation by ID
    fn get_annotation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> impl Future<Output = QdrantResult<Option<Point>>> + Send;

    /// Delete annotation by ID
    fn delete_annotation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Delete multiple annotations by IDs
    fn delete_annotations(
        &self,
        collection_name: &str,
        ids: Vec<PointId>,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Update annotation payload
    fn update_annotation_payload(
        &self,
        collection_name: &str,
        id: PointId,
        payload: serde_json::Value,
    ) -> impl Future<Output = QdrantResult<()>> + Send;

    /// Count annotations by type
    fn count_annotations_by_type(
        &self,
        collection_name: &str,
        annotation_type: AnnotationType,
    ) -> impl Future<Output = QdrantResult<u64>> + Send;
}

impl AnnotationOperations for QdrantClient {
    fn create_annotation_collection(
        &self,
        name: &str,
        config: AnnotationConfig,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move {
            let collection_config = CollectionConfig::new(name)
                .vectors(config.vector_params)
                .replication_factor(1)
                .on_disk_payload(true);

            self.create_collection(collection_config).await
        }
    }

    fn insert_annotation(
        &self,
        collection_name: &str,
        annotation: AnnotationPoint,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move {
            let point: Point = annotation.into();
            self.upsert_point(collection_name, point, true).await
        }
    }

    fn insert_annotations(
        &self,
        collection_name: &str,
        annotations: Vec<AnnotationPoint>,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move {
            let points: Vec<Point> = annotations.into_iter().map(|a| a.into()).collect();
            self.upsert_points(collection_name, points, true).await
        }
    }

    fn search_annotations(
        &self,
        collection_name: &str,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> impl Future<Output = QdrantResult<Vec<SearchResult>>> + Send {
        async move {
            let search_params = params.unwrap_or_default();
            let limit = search_params.limit.unwrap_or(10);

            let request = qdrant_client::qdrant::SearchPoints {
                collection_name: collection_name.to_string(),
                vector: query_vector.values,
                vector_name: None,
                limit,
                score_threshold: search_params.score_threshold,
                offset: None,
                with_payload: Some(if search_params.with_payload {
                    WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(true)),
                    }
                } else {
                    WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(false)),
                    }
                }),
                with_vectors: Some(qdrant_client::qdrant::WithVectorsSelector {
                    selector_options: Some(
                        qdrant_client::qdrant::with_vectors_selector::SelectorOptions::Enable(
                            search_params.with_vectors,
                        ),
                    ),
                }),
                filter: None,
                params: None,
                read_consistency: None,
                shard_key_selector: None,
                sparse_indices: None,
                timeout: None,
            };

            let response = self
                .raw_client()
                .search_points(request)
                .await
                .map_err(QdrantError::Connection)?;

            let results = response
                .result
                .into_iter()
                .map(|scored_point| SearchResult::try_from(scored_point))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| QdrantError::Conversion(e.to_string()))?;

            Ok(results)
        }
    }

    fn search_annotations_by_type(
        &self,
        collection_name: &str,
        annotation_type: AnnotationType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = QdrantResult<Vec<SearchResult>>> + Send {
        async move {
            let search_params = params.unwrap_or_default();
            let limit = search_params.limit.unwrap_or(10);

            // Create filter for annotation type
            let type_filter = Filter {
                must: vec![Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: "annotation_type".to_string(),
                        r#match: Some(Match {
                            match_value: Some(MatchValue::Text(
                                annotation_type.as_str().to_string(),
                            )),
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
                    let request = qdrant_client::qdrant::SearchPoints {
                    collection_name: collection_name.to_string(),
                    vector: vector.values,
                    vector_name: None,
                    limit,
                    score_threshold: search_params.score_threshold,
                    offset: None,
                    with_payload: Some(WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(search_params.with_payload)),
                    }),
                    with_vectors: Some(qdrant_client::qdrant::WithVectorsSelector {
                        selector_options: Some(
                            qdrant_client::qdrant::with_vectors_selector::SelectorOptions::Enable(
                                search_params.with_vectors,
                            ),
                        ),
                    }),
                    filter: Some(type_filter),
                    params: None,
                    read_consistency: None,
                    shard_key_selector: None,
                    sparse_indices: None,
                    timeout: None,
                };

                    let response = self
                        .raw_client()
                        .search_points(request)
                        .await
                        .map_err(QdrantError::Connection)?;

                    let results = response
                        .result
                        .into_iter()
                        .map(|scored_point| SearchResult::try_from(scored_point))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                    Ok(results)
                }
                None => {
                    // Scroll with filter only
                    let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: collection_name.to_string(),
                    filter: Some(type_filter),
                    offset: None,
                    limit: Some(limit as u32),
                    with_payload: Some(WithPayloadSelector {
                        selector_options: Some(SelectorOptions::Enable(search_params.with_payload)),
                    }),
                    with_vectors: Some(qdrant_client::qdrant::WithVectorsSelector {
                        selector_options: Some(
                            qdrant_client::qdrant::with_vectors_selector::SelectorOptions::Enable(
                                search_params.with_vectors,
                            ),
                        ),
                    }),
                    read_consistency: None,
                    order_by: None,
                    shard_key_selector: None,
                    timeout: None,
                };

                    let response = self
                        .raw_client()
                        .scroll(request)
                        .await
                        .map_err(QdrantError::Connection)?;

                    let results = response
                        .result
                        .into_iter()
                        .map(|point| {
                            Point::try_from(point).map(|p| SearchResult::from_point(p, 1.0))
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                    Ok(results)
                }
            }
        }
    }

    fn get_annotation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> impl Future<Output = QdrantResult<Option<Point>>> + Send {
        async move { self.get_point(collection_name, id).await }
    }

    fn delete_annotation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move { self.delete_points(collection_name, vec![id], true).await }
    }

    fn delete_annotations(
        &self,
        collection_name: &str,
        ids: Vec<PointId>,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move { self.delete_points(collection_name, ids, true).await }
    }

    fn update_annotation_payload(
        &self,
        collection_name: &str,
        id: PointId,
        payload: serde_json::Value,
    ) -> impl Future<Output = QdrantResult<()>> + Send {
        async move {
            // Get the existing point
            if let Some(mut point) = self.get_point(collection_name, id.clone()).await? {
                // Update the payload with the new data
                if let serde_json::Value::Object(map) = payload {
                    for (key, value) in map {
                        point.payload.insert(&key, value);
                    }
                }

                // Upsert the updated point
                self.upsert_point(collection_name, point, true).await
            } else {
                Err(QdrantError::PointNotFound {
                    collection: collection_name.to_string(),
                    id: id.to_string(),
                })
            }
        }
    }

    fn count_annotations_by_type(
        &self,
        collection_name: &str,
        annotation_type: AnnotationType,
    ) -> impl Future<Output = QdrantResult<u64>> + Send {
        async move {
            // Create filter for annotation type
            let type_filter = Filter {
                must: vec![Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: "annotation_type".to_string(),
                        r#match: Some(Match {
                            match_value: Some(MatchValue::Text(
                                annotation_type.as_str().to_string(),
                            )),
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
                collection_name: collection_name.to_string(),
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
                .map_err(QdrantError::Connection)?;

            Ok(response.result.map(|r| r.count).unwrap_or(0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Distance;

    #[test]
    fn test_annotation_config_creation() {
        let config = AnnotationConfig::new(384, Distance::Cosine);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert!(!config.text_optimized);
        assert!(!config.image_optimized);
        assert!(!config.spatial_indexing);
    }

    #[test]
    fn test_text_optimized_config() {
        let config = AnnotationConfig::for_text_annotations(384);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(false));
        assert!(config.text_optimized);
        assert!(!config.image_optimized);
        assert!(!config.spatial_indexing);
    }

    #[test]
    fn test_image_optimized_config() {
        let config = AnnotationConfig::for_image_annotations(512);
        assert_eq!(config.vector_params.size, 512);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(true));
        assert!(!config.text_optimized);
        assert!(config.image_optimized);
        assert!(!config.spatial_indexing);
    }

    #[test]
    fn test_spatial_optimized_config() {
        let config = AnnotationConfig::for_spatial_annotations(256);
        assert_eq!(config.vector_params.size, 256);
        assert_eq!(config.vector_params.distance, Distance::Euclid);
        assert!(!config.text_optimized);
        assert!(!config.image_optimized);
        assert!(config.spatial_indexing);
    }

    #[test]
    fn test_annotation_type_string_conversion() {
        assert_eq!(AnnotationType::Text.as_str(), "text");
        assert_eq!(AnnotationType::ImageRegion.as_str(), "image_region");
        assert_eq!(AnnotationType::Text.as_str(), "text");
        assert_eq!(AnnotationType::Code.as_str(), "code");
    }

    #[test]
    fn test_config_builder_methods() {
        let config = AnnotationConfig::new(512, Distance::Euclid)
            .with_spatial_indexing()
            .with_vectors(VectorParams::new(256, Distance::Euclid));

        assert_eq!(config.vector_params.size, 256);
        assert_eq!(config.vector_params.distance, Distance::Euclid);
        assert!(config.spatial_indexing);
    }
}
