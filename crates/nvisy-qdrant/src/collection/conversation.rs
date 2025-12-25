//! Conversation collection operations for QdrantClient.
//!
//! This module provides specialized functionality for managing conversation vectors,
//! including message embeddings, conversation context, participant tracking,
//! and conversation state management.

use std::future::Future;

use qdrant_client::qdrant::condition::ConditionOneOf;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::{FieldCondition, Filter, Match};

use crate::client::QdrantClient;
use crate::collection::SearchParams;
use crate::error::{Error, Result};
use crate::payload::{ConversationPoint, ConversationStatus, MessageType};
use crate::types::{Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Conversation operations trait for QdrantClient.
pub trait ConversationCollection {
    /// Create the conversation collection with sensible defaults
    fn create_collection(&self, vector_size: u64) -> impl Future<Output = Result<()>> + Send;

    /// Delete the conversation collection
    fn delete_collection(&self) -> impl Future<Output = Result<()>> + Send;

    /// Insert a single conversation
    fn insert_conversation(
        &self,
        conversation: ConversationPoint,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Insert multiple conversations in a batch
    fn insert_conversations(
        &self,
        conversations: Vec<ConversationPoint>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Search conversations by vector
    fn search_conversations(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search for conversations by status
    fn search_conversations_by_status(
        &self,
        status: ConversationStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search for conversations by participant
    fn search_conversations_by_participant(
        &self,
        participant_id: &str,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Search for conversations by message type
    fn search_conversations_by_message_type(
        &self,
        message_type: MessageType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> impl Future<Output = Result<Vec<SearchResult>>> + Send;

    /// Get a specific conversation by ID
    fn get_conversation(&self, id: PointId) -> impl Future<Output = Result<Option<Point>>> + Send;

    /// Update conversation status
    fn update_conversation_status(
        &self,
        id: PointId,
        status: ConversationStatus,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Delete conversation by ID
    fn delete_conversation(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Delete multiple conversations by IDs
    fn delete_conversations(&self, ids: Vec<PointId>) -> impl Future<Output = Result<()>> + Send;

    /// Delete a single point by ID
    fn delete_point(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Archive a conversation (mark as archived)
    fn archive_conversation(&self, id: PointId) -> impl Future<Output = Result<()>> + Send;

    /// Count conversations by status
    fn count_conversations_by_status(
        &self,
        status: ConversationStatus,
    ) -> impl Future<Output = Result<u64>> + Send;
}

impl QdrantClient {
    const CONVERSATION_COLLECTION: &'static str = "conversations";
}

impl ConversationCollection for QdrantClient {
    async fn create_collection(&self, vector_size: u64) -> Result<()> {
        use qdrant_client::qdrant::vectors_config::Config;
        use qdrant_client::qdrant::{CreateCollection, VectorsConfig};

        let vector_params = VectorParams::new(vector_size, Distance::Cosine);
        let create_collection = CreateCollection {
            collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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
        self.delete_collection(Self::CONVERSATION_COLLECTION).await
    }

    async fn insert_conversation(&self, conversation: ConversationPoint) -> Result<()> {
        let point: Point = conversation.into();
        self.upsert_point(Self::CONVERSATION_COLLECTION, point)
            .await
    }

    async fn insert_conversations(&self, conversations: Vec<ConversationPoint>) -> Result<()> {
        let points: Vec<Point> = conversations.into_iter().map(|c| c.into()).collect();
        self.upsert_points(Self::CONVERSATION_COLLECTION, points)
            .await
    }

    async fn search_conversations(
        &self,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let request = qdrant_client::qdrant::SearchPoints {
            collection_name: Self::CONVERSATION_COLLECTION.to_string(),
            vector: query_vector.values,
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
            vector_name: None,
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

    async fn search_conversations_by_status(
        &self,
        status: ConversationStatus,
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
                    vector: vector.values,
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
                    vector_name: None,
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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

    async fn search_conversations_by_participant(
        &self,
        participant_id: &str,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let participant_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "participant_id".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(participant_id.to_string())),
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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
                    filter: Some(participant_filter),
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
                    filter: Some(participant_filter),
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

    async fn search_conversations_by_message_type(
        &self,
        message_type: MessageType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> Result<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        let type_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "message_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Text(message_type.as_str().to_string())),
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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
                    collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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

    async fn get_conversation(&self, id: PointId) -> Result<Option<Point>> {
        self.get_point(Self::CONVERSATION_COLLECTION, id).await
    }

    async fn update_conversation_status(
        &self,
        id: PointId,
        status: ConversationStatus,
    ) -> Result<()> {
        if let Some(mut point) = self
            .get_point(Self::CONVERSATION_COLLECTION, id.clone())
            .await?
        {
            point.payload = point.payload.with("status", status.as_str());

            let now = jiff::Timestamp::now().to_string();
            point.payload = point.payload.with("updated_at", now);

            self.upsert_point(Self::CONVERSATION_COLLECTION, point)
                .await
        } else {
            Err(Error::not_found().with_message(format!("Conversation with ID {:?} not found", id)))
        }
    }

    async fn delete_conversation(&self, id: PointId) -> Result<()> {
        self.delete_points(Self::CONVERSATION_COLLECTION, vec![id])
            .await
    }

    async fn delete_conversations(&self, ids: Vec<PointId>) -> Result<()> {
        self.delete_points(Self::CONVERSATION_COLLECTION, ids).await
    }

    async fn delete_point(&self, id: PointId) -> Result<()> {
        self.delete_point(Self::CONVERSATION_COLLECTION, id).await
    }

    async fn archive_conversation(&self, id: PointId) -> Result<()> {
        self.update_conversation_status(id, ConversationStatus::Archived)
            .await
    }

    async fn count_conversations_by_status(&self, status: ConversationStatus) -> Result<u64> {
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
            collection_name: Self::CONVERSATION_COLLECTION.to_string(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_status_string_conversion() {
        assert_eq!(ConversationStatus::Active.as_str(), "active");
        assert_eq!(ConversationStatus::Paused.as_str(), "paused");
        assert_eq!(ConversationStatus::Archived.as_str(), "archived");
        assert_eq!(ConversationStatus::Deleted.as_str(), "deleted");
    }

    #[test]
    fn test_message_type_string_conversion() {
        assert_eq!(MessageType::User.as_str(), "user");
        assert_eq!(MessageType::Media.as_str(), "media");
        assert_eq!(MessageType::Assistant.as_str(), "assistant");
        assert_eq!(MessageType::Tool.as_str(), "tool");
        assert_eq!(MessageType::File.as_str(), "file");
        assert_eq!(MessageType::System.as_str(), "system");
    }
}
