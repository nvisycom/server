//! Document collection operations for QdrantClient.
//!
//! This module provides specialized functionality for managing document vectors,
//! including document content, metadata, full-text search capabilities,
//! author tracking, and document lifecycle management.

use std::future::Future;

use qdrant_client::qdrant::condition::ConditionOneOf;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::{FieldCondition, Filter, Match};

use crate::client::QdrantClient;
use crate::collection::SearchParams;
use crate::error::{Error, Result};
use crate::payload::{DocumentPoint, DocumentStatus, DocumentType};
use crate::types::{Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Document operations trait for QdrantClient.
pub trait DocumentCollection {
    /// Create the document collection with sensible defaults
    fn create_collection(&self, vector_size: u64) -> impl Future<Output = Result<()>> + Send;

    /// Delete the document collection
    fn delete_collection(&self) -> impl Future<Output = Result<()>> + Send;

    /// Insert a document point
    fn insert_document(&self, document: DocumentPoint) -> impl Future<Output = Result<()>> + Send;

    /// Insert multiple document points
    fn insert_documents(
        &self,
        documents: Vec<DocumentPoint>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Search documents by vector
    fn search_documents(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search documents by type
    fn search_documents_by_type(
        &self,
        doc_type: DocumentType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search documents by status
    fn search_documents_by_status(
        &self,
        status: DocumentStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search documents by author
    fn search_documents_by_author(
        &self,
        author_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Get document by ID
    fn get_document(&self, id: PointId) -> impl Future<Output = Result<Option<Point>>> + Send;

    /// Update document status
    fn update_document_status(
        &self,
        id: PointId,
        status: DocumentStatus,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Delete document by ID
    fn delete_document(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Delete multiple documents by ID
    fn delete_documents(&self, ids: Vec<PointId>) -> impl Future<Output = Result<()>> + Send;

    /// Delete a single point by ID
    fn delete_point(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Archive document (set status to archived)
    fn archive_document(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Publish document (set status to published)
    fn publish_document(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Count documents by type
    fn count_documents_by_type(
        &self,
        doc_type: DocumentType,
    ) -> impl Future<Output = Result<u64>> + Send;

    /// Count documents by status
    fn count_documents_by_status(
        &self,
        status: DocumentStatus,
    ) -> impl Future<Output = Result<u64>> + Send;

    /// Count documents by author
    fn count_documents_by_author(
        &self,
        author_id: String,
    ) -> impl Future<Output = Result<u64>> + Send;
}

impl QdrantClient {
    const DOCUMENT_COLLECTION: &'static str = "documents";
}

impl DocumentCollection for QdrantClient {
    async fn create_collection(&self, vector_size: u64) -> Result<()> {
        use qdrant_client::qdrant::vectors_config::Config;
        use qdrant_client::qdrant::{CreateCollection, VectorsConfig};

        let vector_params = VectorParams::new(vector_size, Distance::Cosine);
        let create_collection = CreateCollection {
            collection_name: Self::DOCUMENT_COLLECTION.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(vector_params.to_qdrant_vector_params())),
            }),
            shard_number: None,
            replication_factor: None,
            write_consistency_factor: None,
            on_disk_payload: Some(true),
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
        self.delete_collection(Self::DOCUMENT_COLLECTION).await
    }

    async fn insert_document(&self, document: DocumentPoint) -> Result<()> {
        let point: Point = document.into();
        self.upsert_point(Self::DOCUMENT_COLLECTION, point).await
    }

    async fn insert_documents(&self, documents: Vec<DocumentPoint>) -> Result<()> {
        let points: Vec<Point> = documents.into_iter().map(|d| d.into()).collect();
        self.upsert_points(Self::DOCUMENT_COLLECTION, points).await
    }

    async fn search_documents(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let request = qdrant_client::qdrant::SearchPoints {
            collection_name: Self::DOCUMENT_COLLECTION.to_string(),
            vector: query_vector.values,
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
            .map_err(|e| Error::connection().with_source(Box::new(e)))?;

        let results = response
            .result
            .into_iter()
            .map(SearchResult::try_from)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::serialization().with_message(e.to_string()))?;

        Ok(results)
    }

    async fn search_documents_by_type(
        &self,
        doc_type: DocumentType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let type_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "document_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(doc_type.as_str().to_string())),
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
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results = response
                    .result
                    .into_iter()
                    .map(SearchResult::try_from)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::serialization().with_message(e.to_string()))?;

                Ok(results)
            }
            None => {
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results: Vec<SearchResult> = response
                    .result
                    .into_iter()
                    .filter_map(|point| {
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

    async fn search_documents_by_status(
        &self,
        status: DocumentStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let status_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "status".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(status.as_str().to_string())),
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
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
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
                    filter: Some(status_filter),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results = response
                    .result
                    .into_iter()
                    .map(SearchResult::try_from)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::serialization().with_message(e.to_string()))?;

                Ok(results)
            }
            None => {
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
                    filter: Some(status_filter),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results: Vec<SearchResult> = response
                    .result
                    .into_iter()
                    .filter_map(|point| {
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

    async fn search_documents_by_author(
        &self,
        author_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let author_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "author_id".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(author_id.to_string())),
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
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
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
                    filter: Some(author_filter),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results = response
                    .result
                    .into_iter()
                    .map(SearchResult::try_from)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::serialization().with_message(e.to_string()))?;

                Ok(results)
            }
            None => {
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: Self::DOCUMENT_COLLECTION.to_string(),
                    filter: Some(author_filter),
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
                    .map_err(|e| Error::connection().with_source(Box::new(e)))?;

                let results: Vec<SearchResult> = response
                    .result
                    .into_iter()
                    .filter_map(|point| {
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

    async fn get_document(&self, id: PointId) -> Result<Option<Point>> {
        self.get_point(Self::DOCUMENT_COLLECTION, id).await
    }

    async fn update_document_status(&self, id: PointId, status: DocumentStatus) -> Result<()> {
        if let Some(mut point) = self
            .get_point(Self::DOCUMENT_COLLECTION, id.clone())
            .await?
        {
            point.payload = point.payload.with("status", status.as_str());

            let now = jiff::Timestamp::now().to_string();
            point.payload = point.payload.with("updated_at", now);

            self.upsert_point(Self::DOCUMENT_COLLECTION, point).await
        } else {
            Err(Error::not_found().with_message(format!("Document with ID {:?} not found", id)))
        }
    }

    async fn delete_document(&self, id: PointId) -> Result<()> {
        self.delete_points(Self::DOCUMENT_COLLECTION, vec![id])
            .await
    }

    async fn delete_documents(&self, ids: Vec<PointId>) -> Result<()> {
        self.delete_points(Self::DOCUMENT_COLLECTION, ids).await
    }

    async fn delete_point(&self, id: PointId) -> Result<()> {
        self.delete_point(Self::DOCUMENT_COLLECTION, id).await
    }

    async fn archive_document(&self, id: PointId) -> Result<()> {
        self.update_document_status(id, DocumentStatus::Archived)
            .await
    }

    async fn publish_document(&self, id: PointId) -> Result<()> {
        self.update_document_status(id, DocumentStatus::Published)
            .await
    }

    async fn count_documents_by_type(&self, doc_type: DocumentType) -> Result<u64> {
        let type_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "document_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(doc_type.as_str().to_string())),
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
            collection_name: Self::DOCUMENT_COLLECTION.to_string(),
            filter: Some(type_filter),
            exact: Some(false),
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

    async fn count_documents_by_status(&self, status: DocumentStatus) -> Result<u64> {
        let status_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "status".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(status.as_str().to_string())),
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
            collection_name: Self::DOCUMENT_COLLECTION.to_string(),
            filter: Some(status_filter),
            exact: Some(false),
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

    async fn count_documents_by_author(&self, author_id: String) -> Result<u64> {
        let author_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "author_id".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(author_id.to_string())),
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
            collection_name: Self::DOCUMENT_COLLECTION.to_string(),
            filter: Some(author_filter),
            exact: Some(false),
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
    fn test_document_status_string_conversion() {
        assert_eq!(DocumentStatus::Draft.as_str(), "draft");
        assert_eq!(DocumentStatus::Published.as_str(), "published");
        assert_eq!(DocumentStatus::Archived.as_str(), "archived");
        assert_eq!(DocumentStatus::Deleted.as_str(), "deleted");
    }

    #[test]
    fn test_document_type_string_conversion() {
        assert_eq!(DocumentType::Text.as_str(), "text");
        assert_eq!(DocumentType::Pdf.as_str(), "pdf");
        assert_eq!(DocumentType::Word.as_str(), "word");
        assert_eq!(DocumentType::Code("rust".to_string()).as_str(), "code");
        assert_eq!(DocumentType::Html.as_str(), "html");
        assert_eq!(DocumentType::Custom("test".to_string()).as_str(), "test");
    }
}
