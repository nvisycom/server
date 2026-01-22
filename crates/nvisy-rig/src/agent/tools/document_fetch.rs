//! Document fetch tool for retrieving documents by ID.

use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

/// A fetched document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// The document ID.
    pub id: String,
    /// The document content.
    pub content: String,
    /// Document title if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Document metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Trait for document fetch implementations.
#[async_trait]
pub trait DocumentFetcher: Send + Sync {
    /// Fetch a document by ID.
    async fn fetch(&self, id: &str) -> Result<Option<Document>, DocumentFetchError>;

    /// Fetch multiple documents by IDs.
    async fn fetch_many(&self, ids: &[String]) -> Result<Vec<Document>, DocumentFetchError>;
}

/// Error type for document fetch operations.
#[derive(Debug, thiserror::Error)]
pub enum DocumentFetchError {
    #[error("document not found: {0}")]
    NotFound(String),
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error("connection error: {0}")]
    Connection(String),
}

/// Arguments for document fetch.
#[derive(Debug, Deserialize)]
pub struct DocumentFetchArgs {
    /// The document ID to fetch.
    #[serde(default)]
    pub id: Option<String>,
    /// Multiple document IDs to fetch.
    #[serde(default)]
    pub ids: Option<Vec<String>>,
}

/// Tool for fetching documents by ID.
pub struct DocumentFetchTool<F> {
    fetcher: Arc<F>,
}

impl<F> DocumentFetchTool<F> {
    /// Creates a new document fetch tool.
    pub fn new(fetcher: F) -> Self {
        Self {
            fetcher: Arc::new(fetcher),
        }
    }

    /// Creates a new document fetch tool from an Arc.
    pub fn from_arc(fetcher: Arc<F>) -> Self {
        Self { fetcher }
    }
}

impl<F: DocumentFetcher + 'static> Tool for DocumentFetchTool<F> {
    type Args = DocumentFetchArgs;
    type Error = DocumentFetchError;
    type Output = Vec<Document>;

    const NAME: &'static str = "document_fetch";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Fetch one or more documents by their IDs. Use this to retrieve the full content of documents you've found through search.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "A single document ID to fetch"
                    },
                    "ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Multiple document IDs to fetch"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match (args.id, args.ids) {
            (Some(id), _) => {
                let doc = self
                    .fetcher
                    .fetch(&id)
                    .await?
                    .ok_or(DocumentFetchError::NotFound(id))?;
                Ok(vec![doc])
            }
            (None, Some(ids)) => self.fetcher.fetch_many(&ids).await,
            (None, None) => Ok(vec![]),
        }
    }
}
