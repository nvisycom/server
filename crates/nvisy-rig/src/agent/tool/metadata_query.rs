//! Metadata query tool for filtering documents by metadata.

use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

/// A metadata filter condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFilter {
    /// The field name to filter on.
    pub field: String,
    /// The operator to use.
    pub operator: FilterOperator,
    /// The value to compare against.
    pub value: serde_json::Value,
}

/// Filter operators for metadata queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    /// Equals.
    Eq,
    /// Not equals.
    Ne,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Contains (for arrays or strings).
    Contains,
    /// Starts with (for strings).
    StartsWith,
    /// Ends with (for strings).
    EndsWith,
    /// In (value is in array).
    In,
    /// Not in (value is not in array).
    NotIn,
    /// Exists (field exists).
    Exists,
}

/// Result from a metadata query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// The document ID.
    pub id: String,
    /// The document content (may be truncated).
    pub content: String,
    /// The matched metadata fields.
    pub metadata: serde_json::Value,
}

/// Trait for metadata query implementations.
#[async_trait]
pub trait MetadataQuerier: Send + Sync {
    /// Query documents by metadata filters.
    async fn query(
        &self,
        filters: &[MetadataFilter],
        limit: usize,
        offset: usize,
    ) -> Result<Vec<QueryResult>, MetadataQueryError>;
}

/// Error type for metadata query operations.
#[derive(Debug, thiserror::Error)]
pub enum MetadataQueryError {
    #[error("invalid filter: {0}")]
    InvalidFilter(String),
    #[error("query failed: {0}")]
    Query(String),
    #[error("connection error: {0}")]
    Connection(String),
}

/// Arguments for metadata query.
#[derive(Debug, Deserialize)]
pub struct MetadataQueryArgs {
    /// The filters to apply.
    pub filters: Vec<MetadataFilter>,
    /// Maximum number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Number of results to skip.
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    10
}

/// Tool for querying documents by metadata.
pub struct MetadataQueryTool<Q> {
    querier: Arc<Q>,
}

impl<Q> MetadataQueryTool<Q> {
    /// Creates a new metadata query tool.
    pub fn new(querier: Q) -> Self {
        Self {
            querier: Arc::new(querier),
        }
    }

    /// Creates a new metadata query tool from an Arc.
    pub fn from_arc(querier: Arc<Q>) -> Self {
        Self { querier }
    }
}

impl<Q: MetadataQuerier + 'static> Tool for MetadataQueryTool<Q> {
    type Args = MetadataQueryArgs;
    type Error = MetadataQueryError;
    type Output = Vec<QueryResult>;

    const NAME: &'static str = "metadata_query";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Query documents by their metadata fields. Use this to filter documents by specific attributes like date, author, type, tags, etc.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "filters": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "field": {
                                    "type": "string",
                                    "description": "The metadata field name"
                                },
                                "operator": {
                                    "type": "string",
                                    "enum": ["eq", "ne", "gt", "gte", "lt", "lte", "contains", "starts_with", "ends_with", "in", "not_in", "exists"],
                                    "description": "The comparison operator"
                                },
                                "value": {
                                    "description": "The value to compare against"
                                }
                            },
                            "required": ["field", "operator", "value"]
                        },
                        "description": "The filter conditions to apply"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 10)",
                        "default": 10
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of results to skip for pagination",
                        "default": 0
                    }
                },
                "required": ["filters"]
            }),
        }
    }

    #[tracing::instrument(skip(self), fields(tool = Self::NAME, filter_count = args.filters.len(), limit = args.limit, offset = args.offset))]
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let results = self
            .querier
            .query(&args.filters, args.limit, args.offset)
            .await?;
        tracing::debug!(result_count = results.len(), "metadata_query completed");
        Ok(results)
    }
}
