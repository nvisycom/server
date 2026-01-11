//! Tool module for agent capabilities.
//!
//! This module provides tool support for agents, building on rig-core's
//! tool infrastructure while adding document-specific tools and registry.
//!
//! ## Rig-core integration
//!
//! We re-export key types from rig-core:
//! - [`rig::tool::Tool`] - The core tool trait
//! - [`rig::tool::ToolDyn`] - Dynamic dispatch wrapper
//! - [`rig::completion::ToolDefinition`] - Tool schema definition
//!
//! ## Document tools
//!
//! Pre-built tools for document processing:
//! - `search` - Search document content
//! - `read` - Read specific sections
//! - `extract` - Extract elements (tables, figures)
//! - `edit` - Modify document content
//! - `insert` - Add new content
//! - `redact` - Redact sensitive information
//!
//! ## Submodules
//!
//! - [`edit`] - Proposed edits and edit operations

mod definition;
pub mod edit;
mod registry;
mod types;

// Re-export rig-core tool types
// Our extensions
pub use definition::ToolDefinition;
pub use registry::ToolRegistry;
pub use rig::tool::{Tool, ToolDyn, ToolError};
use serde::{Deserialize, Serialize};
pub use types::{ToolInput, ToolOutput};
use uuid::Uuid;

/// A tool call made by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this call.
    pub id: Uuid,

    /// Name of the tool being called.
    pub name: String,

    /// Arguments to the tool (JSON).
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Creates a new tool call.
    pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            arguments,
        }
    }

    /// Returns whether this tool call is idempotent.
    pub fn is_idempotent(&self) -> bool {
        matches!(self.name.as_str(), "search" | "extract" | "read")
    }

    /// Returns the arguments as a JSON string.
    pub fn arguments_string(&self) -> String {
        serde_json::to_string(&self.arguments).unwrap_or_default()
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The call ID this result is for.
    pub call_id: Uuid,

    /// Whether the call succeeded.
    pub success: bool,

    /// Output from the tool.
    pub output: ToolOutput,

    /// Error message if failed.
    pub error: Option<String>,
}

impl ToolResult {
    /// Creates a successful result.
    pub fn success(call_id: Uuid, output: ToolOutput) -> Self {
        Self {
            call_id,
            success: true,
            output,
            error: None,
        }
    }

    /// Creates a failed result.
    pub fn failure(call_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            call_id,
            success: false,
            output: ToolOutput::empty(),
            error: Some(error.into()),
        }
    }

    /// Creates a result from a rig tool error.
    pub fn from_error(call_id: Uuid, error: ToolError) -> Self {
        Self::failure(call_id, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_idempotency() {
        let search = ToolCall::new("search", serde_json::json!({"query": "test"}));
        let edit = ToolCall::new("edit", serde_json::json!({"content": "new"}));

        assert!(search.is_idempotent());
        assert!(!edit.is_idempotent());
    }

    #[test]
    fn tool_result_success() {
        let call_id = Uuid::now_v7();
        let result = ToolResult::success(call_id, ToolOutput::text("done"));

        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn tool_result_failure() {
        let call_id = Uuid::now_v7();
        let result = ToolResult::failure(call_id, "something went wrong");

        assert!(!result.success);
        assert_eq!(result.error, Some("something went wrong".to_string()));
    }
}
