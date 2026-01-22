//! Scratchpad tool for temporary working storage.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Trait for scratchpad implementations.
#[async_trait]
pub trait Scratchpad: Send + Sync {
    /// Write to the scratchpad.
    async fn write(&self, content: &str) -> Result<(), ScratchpadError>;

    /// Append to the scratchpad.
    async fn append(&self, content: &str) -> Result<(), ScratchpadError>;

    /// Read the scratchpad content.
    async fn read(&self) -> Result<String, ScratchpadError>;

    /// Clear the scratchpad.
    async fn clear(&self) -> Result<(), ScratchpadError>;

    /// Get a named section from the scratchpad.
    async fn get_section(&self, name: &str) -> Result<Option<String>, ScratchpadError>;

    /// Set a named section in the scratchpad.
    async fn set_section(&self, name: &str, content: &str) -> Result<(), ScratchpadError>;
}

/// In-memory scratchpad implementation.
pub struct InMemoryScratchpad {
    content: RwLock<String>,
    sections: RwLock<HashMap<String, String>>,
}

impl InMemoryScratchpad {
    /// Creates a new empty scratchpad.
    pub fn new() -> Self {
        Self {
            content: RwLock::new(String::new()),
            sections: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryScratchpad {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scratchpad for InMemoryScratchpad {
    async fn write(&self, content: &str) -> Result<(), ScratchpadError> {
        let mut guard = self.content.write().await;
        *guard = content.to_string();
        Ok(())
    }

    async fn append(&self, content: &str) -> Result<(), ScratchpadError> {
        let mut guard = self.content.write().await;
        guard.push_str(content);
        Ok(())
    }

    async fn read(&self) -> Result<String, ScratchpadError> {
        let guard = self.content.read().await;
        Ok(guard.clone())
    }

    async fn clear(&self) -> Result<(), ScratchpadError> {
        let mut guard = self.content.write().await;
        guard.clear();
        let mut sections = self.sections.write().await;
        sections.clear();
        Ok(())
    }

    async fn get_section(&self, name: &str) -> Result<Option<String>, ScratchpadError> {
        let guard = self.sections.read().await;
        Ok(guard.get(name).cloned())
    }

    async fn set_section(&self, name: &str, content: &str) -> Result<(), ScratchpadError> {
        let mut guard = self.sections.write().await;
        guard.insert(name.to_string(), content.to_string());
        Ok(())
    }
}

/// Error type for scratchpad operations.
#[derive(Debug, thiserror::Error)]
pub enum ScratchpadError {
    #[error("write failed: {0}")]
    Write(String),
    #[error("read failed: {0}")]
    Read(String),
}

/// The operation to perform on the scratchpad.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScratchpadOperation {
    /// Write content (replaces existing).
    Write { content: String },
    /// Append content.
    Append { content: String },
    /// Read all content.
    Read,
    /// Clear all content.
    Clear,
    /// Get a named section.
    GetSection { name: String },
    /// Set a named section.
    SetSection { name: String, content: String },
}

/// Arguments for scratchpad operations.
#[derive(Debug, Deserialize)]
pub struct ScratchpadArgs {
    /// The operation to perform.
    pub operation: ScratchpadOperation,
}

/// Result of a scratchpad operation.
#[derive(Debug, Serialize)]
pub struct ScratchpadResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// The content (for read operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Optional message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Tool for temporary working storage.
pub struct ScratchpadTool<S> {
    scratchpad: Arc<S>,
}

impl<S> ScratchpadTool<S> {
    /// Creates a new scratchpad tool.
    pub fn new(scratchpad: S) -> Self {
        Self {
            scratchpad: Arc::new(scratchpad),
        }
    }

    /// Creates a new scratchpad tool from an Arc.
    pub fn from_arc(scratchpad: Arc<S>) -> Self {
        Self { scratchpad }
    }
}

impl ScratchpadTool<InMemoryScratchpad> {
    /// Creates a new scratchpad tool with in-memory storage.
    pub fn in_memory() -> Self {
        Self::new(InMemoryScratchpad::new())
    }
}

impl<S: Scratchpad + 'static> Tool for ScratchpadTool<S> {
    const NAME: &'static str = "scratchpad";

    type Error = ScratchpadError;
    type Args = ScratchpadArgs;
    type Output = ScratchpadResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "A temporary workspace for drafting, editing, and organizing content. Use this to work on intermediate results before producing final output.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "object",
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "write": {
                                        "type": "object",
                                        "properties": {
                                            "content": { "type": "string" }
                                        },
                                        "required": ["content"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "append": {
                                        "type": "object",
                                        "properties": {
                                            "content": { "type": "string" }
                                        },
                                        "required": ["content"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "read": { "type": "object" }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "clear": { "type": "object" }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "get_section": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" }
                                        },
                                        "required": ["name"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "set_section": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" },
                                            "content": { "type": "string" }
                                        },
                                        "required": ["name", "content"]
                                    }
                                }
                            }
                        ],
                        "description": "The operation: write, append, read, clear, get_section, or set_section"
                    }
                },
                "required": ["operation"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match args.operation {
            ScratchpadOperation::Write { content } => {
                self.scratchpad.write(&content).await?;
                Ok(ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Content written to scratchpad".to_string()),
                })
            }
            ScratchpadOperation::Append { content } => {
                self.scratchpad.append(&content).await?;
                Ok(ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Content appended to scratchpad".to_string()),
                })
            }
            ScratchpadOperation::Read => {
                let content = self.scratchpad.read().await?;
                Ok(ScratchpadResult {
                    success: true,
                    content: Some(content),
                    message: None,
                })
            }
            ScratchpadOperation::Clear => {
                self.scratchpad.clear().await?;
                Ok(ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Scratchpad cleared".to_string()),
                })
            }
            ScratchpadOperation::GetSection { name } => {
                let content = self.scratchpad.get_section(&name).await?;
                Ok(ScratchpadResult {
                    success: content.is_some(),
                    content,
                    message: None,
                })
            }
            ScratchpadOperation::SetSection { name, content } => {
                self.scratchpad.set_section(&name, &content).await?;
                Ok(ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some(format!("Section '{name}' updated")),
                })
            }
        }
    }
}
