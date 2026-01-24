//! Vector search tool for semantic similarity search.

use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

/// Result from a vector search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The document/chunk ID.
    pub id: String,
    /// The text content.
    pub content: String,
    /// Similarity score (0.0 to 1.0).
    pub score: f64,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Trait for vector search implementations.
#[async_trait]
pub trait VectorSearcher: Send + Sync {
    /// Search for similar documents.
    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: Option<f64>,
    ) -> Result<Vec<SearchResult>, VectorSearchError>;
}

/// Error type for vector search operations.
#[derive(Debug, thiserror::Error)]
pub enum VectorSearchError {
    #[error("embedding failed: {0}")]
    Embedding(String),
    #[error("search failed: {0}")]
    Search(String),
    #[error("connection error: {0}")]
    Connection(String),
}

/// Arguments for vector search.
#[derive(Debug, Deserialize)]
pub struct VectorSearchArgs {
    /// The search query text.
    pub query: String,
    /// Maximum number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Minimum similarity threshold (0.0 to 1.0).
    #[serde(default)]
    pub threshold: Option<f64>,
}

fn default_limit() -> usize {
    5
}

/// Tool for searching vector stores.
pub struct VectorSearchTool<S> {
    searcher: Arc<S>,
}

impl<S> VectorSearchTool<S> {
    /// Creates a new vector search tool.
    pub fn new(searcher: S) -> Self {
        Self {
            searcher: Arc::new(searcher),
        }
    }

    /// Creates a new vector search tool from an Arc.
    pub fn from_arc(searcher: Arc<S>) -> Self {
        Self { searcher }
    }
}

impl<S: VectorSearcher + 'static> Tool for VectorSearchTool<S> {
    type Args = VectorSearchArgs;
    type Error = VectorSearchError;
    type Output = Vec<SearchResult>;

    const NAME: &'static str = "vector_search";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search for semantically similar documents or chunks using vector embeddings. Returns the most relevant results based on meaning, not just keywords.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query text to find similar documents"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 5)",
                        "default": 5
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Minimum similarity score threshold (0.0 to 1.0)",
                        "minimum": 0.0,
                        "maximum": 1.0
                    }
                },
                "required": ["query"]
            }),
        }
    }

    #[tracing::instrument(skip(self), fields(tool = Self::NAME, query_len = args.query.len(), limit = args.limit, threshold = ?args.threshold))]
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let results = self
            .searcher
            .search(&args.query, args.limit, args.threshold)
            .await?;
        tracing::debug!(result_count = results.len(), "vector_search completed");
        Ok(results)
    }
}
