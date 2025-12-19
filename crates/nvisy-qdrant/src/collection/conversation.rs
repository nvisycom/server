//! Conversation collection operations for QdrantClient.
//!
//! This module provides specialized functionality for managing conversation vectors,
//! including message embeddings, conversation context, participant tracking,
//! and conversation state management.

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
use crate::payload::{ConversationPoint, ConversationStatus, MessageType};
use crate::types::{CollectionConfig, Distance, Point, PointId, Vector, VectorParams};
use crate::{Condition, SearchResult, WithPayloadSelector};

/// Configuration for conversation collections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct ConversationConfig {
    /// Vector parameters for the collection
    pub vector_params: VectorParams,
    /// Maximum conversation history to maintain
    pub max_history: Option<usize>,
    /// Enable semantic search on conversation content
    pub semantic_search: bool,
    /// Enable participant indexing
    pub participant_indexing: bool,
    /// Enable temporal indexing for conversation timeline
    pub temporal_indexing: bool,
}

impl ConversationConfig {
    /// Create a new conversation configuration
    pub fn new(dimensions: u64, distance: Distance) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, distance),
            max_history: None,
            semantic_search: true,
            participant_indexing: true,
            temporal_indexing: false,
        }
    }

    /// Create configuration optimized for chat conversations
    pub fn for_chat_conversations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(false),
            max_history: Some(1000), // Keep last 1000 messages
            semantic_search: true,
            participant_indexing: true,
            temporal_indexing: true,
        }
    }

    /// Create configuration optimized for document conversations
    pub fn for_document_conversations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine).on_disk(true),
            max_history: None, // Keep all history for documents
            semantic_search: true,
            participant_indexing: false,
            temporal_indexing: true,
        }
    }

    /// Create configuration for support conversations
    pub fn for_support_conversations(dimensions: u64) -> Self {
        Self {
            vector_params: VectorParams::new(dimensions, Distance::Cosine),
            max_history: Some(500),
            semantic_search: true,
            participant_indexing: true,
            temporal_indexing: true,
        }
    }

    /// Set maximum conversation history
    pub fn max_history(mut self, max: usize) -> Self {
        self.max_history = Some(max);
        self
    }

    /// Enable semantic search
    pub fn with_semantic_search(mut self) -> Self {
        self.semantic_search = true;
        self
    }

    /// Enable participant indexing
    pub fn with_participant_indexing(mut self) -> Self {
        self.participant_indexing = true;
        self
    }

    /// Enable temporal indexing
    pub fn with_temporal_indexing(mut self) -> Self {
        self.temporal_indexing = true;
        self
    }

    /// Set vector parameters
    pub fn with_vectors(mut self, vector_params: VectorParams) -> Self {
        self.vector_params = vector_params;
        self
    }
}

/// Conversation statistics for analytics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct ConversationStats {
    /// Total number of conversations
    pub total_conversations: u64,
    /// Active conversations count
    pub active_conversations: u64,
    /// Archived conversations count
    pub archived_conversations: u64,
    /// Average messages per conversation
    pub avg_messages_per_conversation: f64,
    /// Total participants across all conversations
    pub total_participants: u64,
}

/// Conversation operations trait for QdrantClient.
pub trait ConversationCollection {
    /// Default collection name for conversations
    const DEFAULT_COLLECTION: &'static str = "conversations";

    /// Create a conversation collection
    async fn create_conversation_collection(
        &self,
        name: &str,
        config: ConversationConfig,
    ) -> QdrantResult<()>;

    /// Insert a conversation point
    async fn insert_conversation(
        &self,
        collection_name: &str,
        conversation: ConversationPoint,
    ) -> QdrantResult<()>;

    /// Insert multiple conversation points
    async fn insert_conversations(
        &self,
        collection_name: &str,
        conversations: Vec<ConversationPoint>,
    ) -> QdrantResult<()>;

    /// Search conversations by vector
    async fn search_conversations(
        &self,
        collection_name: &str,
        query_vector: Vector,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search conversations by status
    async fn search_conversations_by_status(
        &self,
        collection_name: &str,
        status: ConversationStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search conversations by participant
    async fn search_conversations_by_participant(
        &self,
        collection_name: &str,
        participant_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Search conversations by message type
    async fn search_conversations_by_message_type(
        &self,
        collection_name: &str,
        message_type: MessageType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>>;

    /// Get conversation by ID
    async fn get_conversation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> QdrantResult<Option<Point>>;

    /// Update conversation status
    async fn update_conversation_status(
        &self,
        collection_name: &str,
        id: PointId,
        status: ConversationStatus,
    ) -> QdrantResult<()>;

    /// Delete conversation by ID
    async fn delete_conversation(&self, collection_name: &str, id: PointId) -> QdrantResult<()>;

    /// Delete multiple conversations by ID
    async fn delete_conversations(
        &self,
        collection_name: &str,
        ids: Vec<PointId>,
    ) -> QdrantResult<()>;

    /// Archive conversation (set status to archived)
    async fn archive_conversation(&self, collection_name: &str, id: PointId) -> QdrantResult<()>;

    /// Get conversation statistics
    async fn get_conversation_stats(
        &self,
        collection_name: &str,
    ) -> QdrantResult<ConversationStats>;

    /// Count conversations by status
    async fn count_conversations_by_status(
        &self,
        collection_name: &str,
        status: ConversationStatus,
    ) -> QdrantResult<u64>;
}

impl ConversationCollection for QdrantClient {
    async fn create_conversation_collection(
        &self,
        name: &str,
        config: ConversationConfig,
    ) -> QdrantResult<()> {
        let collection_config = CollectionConfig::new(name)
            .vectors(config.vector_params)
            .replication_factor(1)
            .on_disk_payload(true);

        self.create_collection(collection_config).await
    }

    async fn insert_conversation(
        &self,
        collection_name: &str,
        conversation: ConversationPoint,
    ) -> QdrantResult<()> {
        let point: Point = conversation.into();
        self.upsert_point(collection_name, point, true).await
    }

    async fn insert_conversations(
        &self,
        collection_name: &str,
        conversations: Vec<ConversationPoint>,
    ) -> QdrantResult<()> {
        let points: Vec<Point> = conversations.into_iter().map(|c| c.into()).collect();
        self.upsert_points(collection_name, points, true).await
    }

    async fn search_conversations(
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
            .map_err(QdrantError::Connection)?;

        let results = response
            .result
            .into_iter()
            .map(|scored_point| SearchResult::try_from(scored_point))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| QdrantError::Conversion(e.to_string()))?;

        Ok(results)
    }

    async fn search_conversations_by_status(
        &self,
        collection_name: &str,
        status: ConversationStatus,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for conversation status
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

    async fn search_conversations_by_participant(
        &self,
        collection_name: &str,
        participant_id: String,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for participant
        let participant_filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "conversation_id".to_string(),
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

    async fn search_conversations_by_message_type(
        &self,
        collection_name: &str,
        message_type: MessageType,
        query_vector: Option<Vector>,
        params: Option<SearchParams>,
    ) -> QdrantResult<Vec<SearchResult>> {
        let search_params = params.unwrap_or_default();
        let limit = search_params.limit.unwrap_or(10);

        // Create filter for message type
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
                    .map(|point| Point::try_from(point).map(|p| SearchResult::from_point(p, 1.0)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| QdrantError::Conversion(e.to_string()))?;

                Ok(results)
            }
        }
    }

    async fn get_conversation(
        &self,
        collection_name: &str,
        id: PointId,
    ) -> QdrantResult<Option<Point>> {
        self.get_point(collection_name, id).await
    }

    async fn update_conversation_status(
        &self,
        collection_name: &str,
        id: PointId,
        status: ConversationStatus,
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

    async fn delete_conversation(&self, collection_name: &str, id: PointId) -> QdrantResult<()> {
        self.delete_points(collection_name, vec![id], true).await
    }

    async fn delete_conversations(
        &self,
        collection_name: &str,
        ids: Vec<PointId>,
    ) -> QdrantResult<()> {
        self.delete_points(collection_name, ids, true).await
    }

    async fn archive_conversation(&self, collection_name: &str, id: PointId) -> QdrantResult<()> {
        self.update_conversation_status(collection_name, id, ConversationStatus::Archived)
            .await
    }

    async fn get_conversation_stats(
        &self,
        collection_name: &str,
    ) -> QdrantResult<ConversationStats> {
        // Get collection info for total count
        let info = self.collection_info(collection_name).await?;
        let total_conversations = info.points_count;

        // Count by status
        let active_count = self
            .count_conversations_by_status(collection_name, ConversationStatus::Active)
            .await
            .unwrap_or(0);
        let archived_count = self
            .count_conversations_by_status(collection_name, ConversationStatus::Archived)
            .await
            .unwrap_or(0);

        // For now, set defaults for other statistics
        // In a real implementation, you might want to calculate these properly
        let avg_messages_per_conversation = 5.0; // Placeholder
        let total_participants = total_conversations; // Placeholder

        Ok(ConversationStats {
            total_conversations: total_conversations.unwrap_or(0),
            active_conversations: active_count,
            archived_conversations: archived_count,
            avg_messages_per_conversation,
            total_participants: total_participants.unwrap_or(0),
        })
    }

    async fn count_conversations_by_status(
        &self,
        collection_name: &str,
        status: ConversationStatus,
    ) -> QdrantResult<u64> {
        // Create filter for conversation status
        // Create filter for status
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
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Distance;

    #[test]
    fn test_conversation_config_creation() {
        let config = ConversationConfig::new(384, Distance::Cosine);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert!(config.semantic_search);
        assert!(config.participant_indexing);
        assert!(!config.temporal_indexing);
        assert_eq!(config.max_history, None);
    }

    #[test]
    fn test_chat_optimized_config() {
        let config = ConversationConfig::for_chat_conversations(384);
        assert_eq!(config.vector_params.size, 384);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(false));
        assert_eq!(config.max_history, Some(1000));
        assert!(config.semantic_search);
        assert!(config.participant_indexing);
        assert!(config.temporal_indexing);
    }

    #[test]
    fn test_document_optimized_config() {
        let config = ConversationConfig::for_document_conversations(512);
        assert_eq!(config.vector_params.size, 512);
        assert_eq!(config.vector_params.distance, Distance::Cosine);
        assert_eq!(config.vector_params.on_disk, Some(true));
        assert_eq!(config.max_history, None);
        assert!(config.semantic_search);
        assert!(!config.participant_indexing);
        assert!(config.temporal_indexing);
    }

    #[test]
    fn test_support_optimized_config() {
        let config = ConversationConfig::for_support_conversations(256);
        assert_eq!(config.vector_params.size, 256);
        assert_eq!(config.max_history, Some(500));
        assert!(config.semantic_search);
        assert!(config.participant_indexing);
        assert!(config.temporal_indexing);
    }

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

    #[test]
    fn test_config_builder_methods() {
        let config = ConversationConfig::new(128, Distance::Cosine)
            .max_history(200)
            .with_semantic_search()
            .with_participant_indexing()
            .with_temporal_indexing();

        assert_eq!(config.max_history, Some(200));
        assert!(config.semantic_search);
        assert!(config.participant_indexing);
        assert!(config.temporal_indexing);
    }
}
