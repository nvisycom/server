//! Document collection operations for QdrantClient.
//!
//! This module provides specialized functionality for managing document vectors,
//! including document content, metadata, full-text search capabilities,
//! author tracking, and document lifecycle management.

use qdrant_client::qdrant::condition::ConditionOneOf;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::{FieldCondition, Filter, Match};
use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

use crate::client::QdrantClient;
use crate::collection::SearchParams;
use crate::error::{QdrantError, QdrantResult};
use crate::payload::{DocumentPoint, DocumentStatus, DocumentType};
use crate::types::{CollectionConfig, Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Configuration for document collections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct DocumentConfig {
    /// Vector parameters for the collection
    pub vector_params: VectorParams,
    /// Enable full-text search indexing
    pub full_text_search: bool,
    /// Maximum document chunk size for processing
    pub max_chunk_size: Option<usize>,
    /// Enable version tracking
    pub version_tracking: bool,
    /// Enable author indexing
    pub author_indexing: bool,
    /// Enable content type indexing
    pub content_type_indexing: bool,
}

impl DocumentConfig {
    /// Create a new document configuration
    pub fn new(dimensions: u64, distance: Distance) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, distance),
            full_text_search: true,
            max_chunk_size: None,
            version_tracking: false,
            author_indexing: true,
            content_type_indexing: true,
        }
    }

    /// Create configuration optimized for text documents
    pub fn for_text_documents(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(false),
            full_text_search: true,
            max_chunk_size: Some(1000), // 1000 tokens per chunk
            version_tracking: true,
            author_indexing: true,
            content_type_indexing: true,
        }
    }

    /// Create configuration optimized for large documents (PDFs, etc.)
    pub fn for_large_documents(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(true),
            full_text_search: true,
            max_chunk_size: Some(500), // Smaller chunks for large docs
            version_tracking: true,
            author_indexing: true,
            content_type_indexing: true,
        }
    }

    /// Create configuration optimized for code documents
    pub fn for_code_documents(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(false),
            full_text_search: true,
            max_chunk_size: Some(2000), // Larger chunks for code
            version_tracking: true,
            author_indexing: true,
            content_type_indexing: true,
        }
    }

    /// Create configuration for multimedia documents
    pub fn for_multimedia_documents(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(true),
            full_text_search: false, // No full-text for multimedia
            max_chunk_size: None,
            version_tracking: false,
            author_indexing: true,
            content_type_indexing: true,
        }
    }

    /// Enable full-text search
    pub fn with_full_text_search(mut self) -> Self {
        self.full_text_search = true;
        self
    }

    /// Set maximum chunk size
    pub fn max_chunk_size(mut self, size: usize) -> Self {
        self.max_chunk_size = Some(size);
        self
    }

    /// Enable version tracking
    pub fn with_version_tracking(mut self) -> Self {
        self.version_tracking = true;
        self
    }

    /// Enable author indexing
    pub fn with_author_indexing(mut self) -> Self {
        self.author_indexing = true;
        self
    }

    /// Enable content type indexing
    pub fn with_content_type_indexing(mut self) -> Self {
        self.content_type_indexing = true;
        self
    }

    /// Set vector parameters
    pub fn with_vectors(mut self, vector_params: VectorParams) -> Self {
        self.vector_params = vector_params;
        self
    }
}

/// Document statistics for analytics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct DocumentStats {
    /// Total number of documents
    pub total_documents: u64,
    /// Published documents count
    pub published_documents: u64,
    /// Draft documents count
    pub draft_documents: u64,
    /// Archived documents count
    pub archived_documents: u64,
    /// Total unique authors
    pub total_authors: u64,
    /// Average document size in bytes
    pub avg_document_size: f64,
}

/// Author statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct AuthorStats {
    /// Author identifier
    pub author_id: String,
    /// Number of documents by this author
    pub document_count: u64,
    /// Total size of documents by this author
    pub total_size: u64,
    /// Most recent document timestamp
    pub last_updated: Option<String>,
}

/// Document type statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct DocumentTypeStats {
    /// Document type
    pub document_type: DocumentType,
    /// Number of documents of this type
    pub count: u64,
    /// Total size of documents of this type
    pub total_size: u64,
    /// Average size for this document type
    pub avg_size: f64,
}

/// Document operations trait for QdrantClient.
pub trait DocumentCollection {
    /// Default collection name for documents
    const DEFAULT_COLLECTION: &'static str = "documents";

    /// Create a document collection
    async fn create_document_collection(
        &self,
        name: &str,
        config: DocumentConfig,
    ) -> QdrantResult<()>;

    /// Insert a document point
    async fn insert_document(
        &self,
        collection_name: &str,
        document: DocumentPoint,
    ) -> QdrantResult<()>;

    /// Insert multiple document points
    async fn insert_documents(
        &self,
        collection_name: &str,
        documents: Vec<DocumentPoint>,
    ) -> QdrantResult<()>;

    /// Search documents by vector
    async fn search_documents(
        &self,
        collection_name: &str,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search documents by type
    async fn search_documents_by_type(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search documents by status
    async fn search_documents_by_status(
        &self,
        collection_name: &str,
        status: DocumentStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search documents by author
    async fn search_documents_by_author(
        &self,
        collection_name: &str,
        author_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Get document by ID
    async fn get_document(&self, collection_name: &str, id: PointId)
    -> QdrantResult<Option<Point>>;

    /// Update document status
    async fn update_document_status(
        &self,
        collection_name: &str,
        id: PointId,
        status: DocumentStatus,
    ) -> QdrantResult<()>;

    /// Delete document by ID
    async fn delete_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()>;

    /// Delete multiple documents by ID
    async fn delete_documents(&self, collection_name: &str, ids: Vec<PointId>) -> QdrantResult<()>;

    /// Archive document (set status to archived)
    async fn archive_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()>;

    /// Publish document (set status to published)
    async fn publish_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()>;

    /// Get document statistics
    async fn get_document_stats(&self, collection_name: &str) -> QdrantResult<DocumentStats>;

    /// Get author statistics
    async fn get_author_stats(
        &self,
        collection_name: &str,
        author_id: String,
    ) -> QdrantResult<AuthorStats>;

    /// Get document type statistics
    async fn get_document_type_stats(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
    ) -> QdrantResult<DocumentTypeStats>;

    /// Count documents by type
    async fn count_documents_by_type(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
    ) -> QdrantResult<u64>;

    /// Count documents by status
    async fn count_documents_by_status(
        &self,
        collection_name: &str,
        status: DocumentStatus,
    ) -> QdrantResult<u64>;

    /// Count documents by author
    async fn count_documents_by_author(
        &self,
        collection_name: &str,
        author_id: String,
    ) -> QdrantResult<u64>;
}

impl DocumentCollection for QdrantClient {
    async fn create_document_collection(
        &self,
        name: &str,
        config: DocumentConfig,
    ) -> QdrantResult<()> {
        let collection_config = CollectionConfig::new(name)
            .vectors(config.vector_params)
            .replication_factor(1)
            .on_disk_payload(config.full_text_search); // Use on-disk payload for full-text search

        self.create_collection(collection_config).await
    }

    async fn insert_document(
        &self,
        collection_name: &str,
        document: DocumentPoint,
    ) -> QdrantResult<()> {
        let point: Point = document.into();
        self.upsert_point(collection_name, point, true).await
    }

    async fn insert_documents(
        &self,
        collection_name: &str,
        documents: Vec<DocumentPoint>,
    ) -> QdrantResult<()> {
        let points: Vec<Point> = documents.into_iter().map(|d| d.into()).collect();
        self.upsert_points(collection_name, points, true).await
    }

    async fn search_documents(
        &self,
        collection_name: &str,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let request = qdrant_client::qdrant::SearchPoints {
            collection_name: collection_name.to_string(),
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
            .map_err(QdrantError::Connection)?;

        let results = response
            .result
            .into_iter()
            .map(|scored_point| SearchResult::try_from(scored_point))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| QdrantError::Conversion(e.to_string()))?;

        Ok(results)
    }

    async fn search_documents_by_type(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for document type
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
                    .map(|point| Point::try_from(point).map(|p| SearchResult::from_point(p, 1.0)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                Ok(results)
            }
        }
    }

    async fn search_documents_by_status(
        &self,
        collection_name: &str,
        status: DocumentStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for document status
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
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: collection_name.to_string(),
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
                    .map_err(QdrantError::Connection)?;

                let results = response
                    .result
                    .into_iter()
                    .map(|point| Point::try_from(point).map(|p| SearchResult::from_point(p, 1.0)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                Ok(results)
            }
        }
    }

    async fn search_documents_by_author(
        &self,
        collection_name: &str,
        author_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for author
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
                let request = qdrant_client::qdrant::ScrollPoints {
                    collection_name: collection_name.to_string(),
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
                    .map_err(QdrantError::Connection)?;

                let results = response
                    .result
                    .into_iter()
                    .map(|point| Point::try_from(point).map(|p| SearchResult::from_point(p, 1.0)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                Ok(results)
            }
        }
    }

    async fn get_document(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> QdrantResult<Option<Point>> {
        self.get_point(collection_name, id).await
    }

    async fn update_document_status(
        &self,
        collection_name: &str,
        id: PointId,
        status: DocumentStatus,
    ) -> QdrantResult<()> {
        // Get the existing point
        if let Some(mut point) = self.get_point(collection_name, id.clone()).await? {
            // Update the status in the payload
            point.payload.insert("status", status.as_str());

            // Upsert the updated point
            self.upsert_point(collection_name, point, true).await
        } else {
            Err(QdrantError::PointNotFound {
                collection: collection_name.to_string(),
                id: id.to_string(),
            })
        }
    }

    async fn delete_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()> {
        self.delete_points(collection_name, vec![id], true).await
    }

    async fn delete_documents(&self, collection_name: &str, ids: Vec<PointId>) -> QdrantResult<()> {
        self.delete_points(collection_name, ids, true).await
    }

    async fn archive_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()> {
        self.update_document_status(collection_name, id, DocumentStatus::Archived)
            .await
    }

    async fn publish_document(&self, collection_name: &str, id: PointId) -> QdrantResult<()> {
        self.update_document_status(collection_name, id, DocumentStatus::Published)
            .await
    }

    async fn get_document_stats(&self, collection_name: &str) -> QdrantResult<DocumentStats> {
        // Get collection info for total count
        let info = self.collection_info(collection_name).await?;
        let total_documents = info.points_count;

        // Count by status
        let published_count = self
            .count_documents_by_status(collection_name, DocumentStatus::Published)
            .await
            .unwrap_or(0);
        let draft_count = self
            .count_documents_by_status(collection_name, DocumentStatus::Draft)
            .await
            .unwrap_or(0);
        let archived_count = self
            .count_documents_by_status(collection_name, DocumentStatus::Archived)
            .await
            .unwrap_or(0);

        // Placeholder values - in a real implementation, you might calculate these properly
        let total_authors = total_documents.unwrap_or(0) / 2; // Rough estimate
        let avg_document_size = 5000.0; // Placeholder

        Ok(DocumentStats {
            total_documents: total_documents.unwrap_or(0),
            published_documents: published_count,
            draft_documents: draft_count,
            archived_documents: archived_count,
            total_authors,
            avg_document_size,
        })
    }

    async fn get_author_stats(
        &self,
        collection_name: &str,
        author_id: String,
    ) -> QdrantResult<AuthorStats> {
        let document_count = self
            .count_documents_by_author(collection_name, author_id.clone())
            .await?;

        // Placeholder values - in a real implementation, you might calculate these properly
        let total_size = document_count * 5000; // Rough estimate
        let last_updated = Some("2024-01-01T00:00:00Z".to_string()); // Placeholder

        Ok(AuthorStats {
            author_id,
            document_count,
            total_size,
            last_updated,
        })
    }

    async fn get_document_type_stats(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
    ) -> QdrantResult<DocumentTypeStats> {
        let count = self
            .count_documents_by_type(collection_name, doc_type.clone())
            .await?;

        // Placeholder values - in a real implementation, you might calculate these properly
        let total_size = count * 3000; // Rough estimate
        let avg_size = if count > 0 {
            total_size as f64 / count as f64
        } else {
            0.0
        };

        Ok(DocumentTypeStats {
            document_type: doc_type,
            count,
            total_size,
            avg_size,
        })
    }

    async fn count_documents_by_type(
        &self,
        collection_name: &str,
        doc_type: DocumentType,
    ) -> QdrantResult<u64> {
        // Create filter for document type
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

    async fn count_documents_by_status(
        &self,
        collection_name: &str,
        status: DocumentStatus,
    ) -> QdrantResult<u64> {
        // Create filter for document status
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
            collection_name: collection_name.to_string(),
            filter: Some(status_filter),
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

    async fn count_documents_by_author(
        &self,
        collection_name: &str,
        author_id: String,
    ) -> QdrantResult<u64> {
        // Create filter for author
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
            collection_name: collection_name.to_string(),
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
            .map_err(QdrantError::Connection)?;

        Ok(response.result.map(|r| r.count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Distance;

    #[test]
    fn test_document_config_creation() {
        let config = DocumentConfig::new(384, Distance::Cosine);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert!(config.full_text_search);
        assert!(!config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
        assert_eq!(config.max_chunk_size, None);
    }

    #[test]
    fn test_text_optimized_config() {
        let config = DocumentConfig::for_text_documents(384);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(false));
        assert_eq!(config.max_chunk_size, Some(1000));
        assert!(config.full_text_search);
        assert!(config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
    }

    #[test]
    fn test_large_document_optimized_config() {
        let config = DocumentConfig::for_large_documents(512);
        assert_eq!(config.vector_params.size, 512);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(true));
        assert_eq!(config.max_chunk_size, Some(500));
        assert!(config.full_text_search);
        assert!(config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
    }

    #[test]
    fn test_code_optimized_config() {
        let config = DocumentConfig::for_code_documents(256);
        assert_eq!(config.vector_params.size, 256);
        assert_eq!(config.max_chunk_size, Some(2000));
        assert!(config.full_text_search);
        assert!(config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
    }

    #[test]
    fn test_multimedia_optimized_config() {
        let config = DocumentConfig::for_multimedia_documents(768);
        assert_eq!(config.vector_params.size, 768);
        assert_eq!(config.vector_params.on_disk, Some(true));
        assert_eq!(config.max_chunk_size, None);
        assert!(!config.full_text_search);
        assert!(!config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
    }

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
        assert_eq!(DocumentType::Custom("test".to_string()).as_str(), "custom");
    }

    #[test]
    fn test_config_builder_methods() {
        let config = DocumentConfig::new(128, Distance::Cosine)
            .max_chunk_size(750)
            .with_full_text_search()
            .with_version_tracking()
            .with_author_indexing()
            .with_content_type_indexing();

        assert_eq!(config.max_chunk_size, Some(750));
        assert!(config.full_text_search);
        assert!(config.version_tracking);
        assert!(config.author_indexing);
        assert!(config.content_type_indexing);
    }

    #[test]
    fn test_stats_structures() {
        let doc_stats = DocumentStats {
            total_documents: 100,
            published_documents: 80,
            draft_documents: 15,
            archived_documents: 5,
            total_authors: 10,
            avg_document_size: 5000.0,
        };

        assert_eq!(doc_stats.total_documents, 100);
        assert_eq!(doc_stats.published_documents, 80);
        assert_eq!(doc_stats.draft_documents, 15);
        assert_eq!(doc_stats.archived_documents, 5);

        let author_stats = AuthorStats {
            author_id: "author-123".to_string(),
            document_count: 25,
            total_size: 125000,
            last_updated: Some("2024-01-01T00:00:00Z".to_string()),
        };

        assert_eq!(author_stats.author_id, "author-123");
        assert_eq!(author_stats.document_count, 25);
        assert_eq!(author_stats.total_size, 125000);

        let type_stats = DocumentTypeStats {
            document_type: DocumentType::Text,
            count: 50,
            total_size: 250000,
            avg_size: 5000.0,
        };

        assert_eq!(type_stats.count, 50);
        assert_eq!(type_stats.total_size, 250000);
        assert_eq!(type_stats.avg_size, 5000.0);
    }
}
