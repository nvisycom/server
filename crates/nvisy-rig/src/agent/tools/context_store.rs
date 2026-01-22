//! Context store tool for persistent agent memory.

use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

/// Trait for context store implementations.
#[async_trait]
pub trait ContextStore: Send + Sync {
    /// Store a value with a key.
    async fn set(&self, key: &str, value: serde_json::Value) -> Result<(), ContextStoreError>;

    /// Retrieve a value by key.
    async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, ContextStoreError>;

    /// Delete a value by key.
    async fn delete(&self, key: &str) -> Result<bool, ContextStoreError>;

    /// List all keys with optional prefix filter.
    async fn list(&self, prefix: Option<&str>) -> Result<Vec<String>, ContextStoreError>;
}

/// Error type for context store operations.
#[derive(Debug, thiserror::Error)]
pub enum ContextStoreError {
    #[error("store failed: {0}")]
    Store(String),
    #[error("retrieve failed: {0}")]
    Retrieve(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// The operation to perform on the context store.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextOperation {
    /// Store a value.
    Set {
        key: String,
        value: serde_json::Value,
    },
    /// Retrieve a value.
    Get { key: String },
    /// Delete a value.
    Delete { key: String },
    /// List all keys.
    List { prefix: Option<String> },
}

/// Arguments for context store operations.
#[derive(Debug, Deserialize)]
pub struct ContextStoreArgs {
    /// The operation to perform.
    pub operation: ContextOperation,
}

/// Result of a context store operation.
#[derive(Debug, Serialize)]
pub struct ContextStoreResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// The result value (for get operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// List of keys (for list operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
    /// Optional message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Tool for storing and retrieving context.
pub struct ContextStoreTool<S> {
    store: Arc<S>,
}

impl<S> ContextStoreTool<S> {
    /// Creates a new context store tool.
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Creates a new context store tool from an Arc.
    pub fn from_arc(store: Arc<S>) -> Self {
        Self { store }
    }
}

impl<S: ContextStore + 'static> Tool for ContextStoreTool<S> {
    type Args = ContextStoreArgs;
    type Error = ContextStoreError;
    type Output = ContextStoreResult;

    const NAME: &'static str = "context_store";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Store and retrieve persistent context values. Use this to remember information across conversation turns or save intermediate results.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "object",
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "set": {
                                        "type": "object",
                                        "properties": {
                                            "key": { "type": "string" },
                                            "value": {}
                                        },
                                        "required": ["key", "value"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "get": {
                                        "type": "object",
                                        "properties": {
                                            "key": { "type": "string" }
                                        },
                                        "required": ["key"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "delete": {
                                        "type": "object",
                                        "properties": {
                                            "key": { "type": "string" }
                                        },
                                        "required": ["key"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "list": {
                                        "type": "object",
                                        "properties": {
                                            "prefix": { "type": "string" }
                                        }
                                    }
                                }
                            }
                        ],
                        "description": "The operation to perform: set, get, delete, or list"
                    }
                },
                "required": ["operation"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match args.operation {
            ContextOperation::Set { key, value } => {
                self.store.set(&key, value).await?;
                Ok(ContextStoreResult {
                    success: true,
                    value: None,
                    keys: None,
                    message: Some(format!("Stored value for key: {key}")),
                })
            }
            ContextOperation::Get { key } => {
                let value = self.store.get(&key).await?;
                Ok(ContextStoreResult {
                    success: value.is_some(),
                    value,
                    keys: None,
                    message: None,
                })
            }
            ContextOperation::Delete { key } => {
                let deleted = self.store.delete(&key).await?;
                Ok(ContextStoreResult {
                    success: deleted,
                    value: None,
                    keys: None,
                    message: if deleted {
                        Some(format!("Deleted key: {key}"))
                    } else {
                        Some(format!("Key not found: {key}"))
                    },
                })
            }
            ContextOperation::List { prefix } => {
                let keys = self.store.list(prefix.as_deref()).await?;
                Ok(ContextStoreResult {
                    success: true,
                    value: None,
                    keys: Some(keys),
                    message: None,
                })
            }
        }
    }
}
