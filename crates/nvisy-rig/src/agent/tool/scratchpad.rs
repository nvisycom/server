//! Scratchpad tool for temporary working storage.

use std::collections::HashMap;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Error type for scratchpad operations.
#[derive(Debug, thiserror::Error)]
#[error("scratchpad error")]
pub struct ScratchpadError;

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

/// In-memory scratchpad storage.
struct InMemoryScratchpad {
    content: RwLock<String>,
    sections: RwLock<HashMap<String, String>>,
}

impl InMemoryScratchpad {
    fn new() -> Self {
        Self {
            content: RwLock::new(String::new()),
            sections: RwLock::new(HashMap::new()),
        }
    }

    async fn write(&self, content: &str) {
        let mut guard = self.content.write().await;
        *guard = content.to_string();
    }

    async fn append(&self, content: &str) {
        let mut guard = self.content.write().await;
        guard.push_str(content);
    }

    async fn read(&self) -> String {
        let guard = self.content.read().await;
        guard.clone()
    }

    async fn clear(&self) {
        let mut guard = self.content.write().await;
        guard.clear();
        let mut sections = self.sections.write().await;
        sections.clear();
    }

    async fn get_section(&self, name: &str) -> Option<String> {
        let guard = self.sections.read().await;
        guard.get(name).cloned()
    }

    async fn set_section(&self, name: &str, content: &str) {
        let mut guard = self.sections.write().await;
        guard.insert(name.to_string(), content.to_string());
    }
}

/// Tool for temporary working storage.
///
/// Provides a scratchpad for agents to draft, edit, and organize content
/// during multi-step reasoning tasks.
pub struct ScratchpadTool {
    scratchpad: Arc<InMemoryScratchpad>,
}

impl ScratchpadTool {
    /// Creates a new scratchpad tool with in-memory storage.
    pub fn new() -> Self {
        Self {
            scratchpad: Arc::new(InMemoryScratchpad::new()),
        }
    }
}

impl Default for ScratchpadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ScratchpadTool {
    type Args = ScratchpadArgs;
    type Error = ScratchpadError;
    type Output = ScratchpadResult;

    const NAME: &'static str = "scratchpad";

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

    #[tracing::instrument(skip(self, args), fields(tool = Self::NAME, operation = ?std::mem::discriminant(&args.operation)))]
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = match args.operation {
            ScratchpadOperation::Write { content } => {
                tracing::debug!(content_len = content.len(), "scratchpad write");
                self.scratchpad.write(&content).await;
                ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Content written to scratchpad".to_string()),
                }
            }
            ScratchpadOperation::Append { content } => {
                tracing::debug!(content_len = content.len(), "scratchpad append");
                self.scratchpad.append(&content).await;
                ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Content appended to scratchpad".to_string()),
                }
            }
            ScratchpadOperation::Read => {
                let content = self.scratchpad.read().await;
                tracing::debug!(content_len = content.len(), "scratchpad read");
                ScratchpadResult {
                    success: true,
                    content: Some(content),
                    message: None,
                }
            }
            ScratchpadOperation::Clear => {
                tracing::debug!("scratchpad clear");
                self.scratchpad.clear().await;
                ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some("Scratchpad cleared".to_string()),
                }
            }
            ScratchpadOperation::GetSection { name } => {
                let content = self.scratchpad.get_section(&name).await;
                tracing::debug!(section = %name, found = content.is_some(), "scratchpad get_section");
                ScratchpadResult {
                    success: content.is_some(),
                    content,
                    message: None,
                }
            }
            ScratchpadOperation::SetSection { name, content } => {
                tracing::debug!(section = %name, content_len = content.len(), "scratchpad set_section");
                self.scratchpad.set_section(&name, &content).await;
                ScratchpadResult {
                    success: true,
                    content: None,
                    message: Some(format!("Section '{name}' updated")),
                }
            }
        };
        Ok(result)
    }
}
