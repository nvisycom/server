//! Tool definitions and schemas.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Definition of a tool available to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique name of the tool.
    name: String,

    /// Human-readable description.
    description: String,

    /// JSON Schema for the tool's parameters.
    parameters: Value,

    /// Whether the tool is idempotent.
    idempotent: bool,

    /// Whether the tool requires user confirmation.
    requires_confirmation: bool,
}

impl ToolDefinition {
    /// Creates a new tool definition.
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            idempotent: false,
            requires_confirmation: false,
        }
    }

    /// Marks the tool as idempotent.
    pub fn idempotent(mut self) -> Self {
        self.idempotent = true;
        self
    }

    /// Marks the tool as requiring confirmation.
    pub fn with_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    /// Returns the tool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the tool description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the parameter schema.
    pub fn parameters(&self) -> &Value {
        &self.parameters
    }

    /// Returns whether the tool is idempotent.
    pub fn is_idempotent(&self) -> bool {
        self.idempotent
    }

    /// Returns whether the tool requires confirmation.
    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }

    /// Converts to OpenAI function format.
    pub fn to_openai_function(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters
            }
        })
    }

    /// Converts to Anthropic tool format.
    pub fn to_anthropic_tool(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.parameters
        })
    }
}

/// Builder for common tool definitions.
pub struct ToolBuilder;

impl ToolBuilder {
    /// Creates the search tool definition.
    pub fn search() -> ToolDefinition {
        ToolDefinition::new(
            "search",
            "Search for content within the document",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        )
        .idempotent()
    }

    /// Creates the read tool definition.
    pub fn read() -> ToolDefinition {
        ToolDefinition::new(
            "read",
            "Read a specific section or page of the document",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "section": {
                        "type": "string",
                        "description": "Section identifier (page number, heading, etc.)"
                    },
                    "range": {
                        "type": "object",
                        "properties": {
                            "start": { "type": "integer" },
                            "end": { "type": "integer" }
                        },
                        "description": "Page range to read"
                    }
                }
            }),
        )
        .idempotent()
    }

    /// Creates the extract tool definition.
    pub fn extract() -> ToolDefinition {
        ToolDefinition::new(
            "extract",
            "Extract a specific element from the document (table, figure, etc.)",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "element_type": {
                        "type": "string",
                        "enum": ["table", "figure", "section", "list", "code"],
                        "description": "Type of element to extract"
                    },
                    "identifier": {
                        "type": "string",
                        "description": "Element identifier (e.g., 'Table 12.6', 'Figure 3')"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["markdown", "json", "csv", "text"],
                        "description": "Output format for the extracted content"
                    }
                },
                "required": ["element_type", "identifier"]
            }),
        )
        .idempotent()
    }

    /// Creates the edit tool definition.
    pub fn edit() -> ToolDefinition {
        ToolDefinition::new(
            "edit",
            "Edit content in the document",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "object",
                        "properties": {
                            "page": { "type": "integer" },
                            "section": { "type": "string" },
                            "offset": { "type": "integer" }
                        },
                        "description": "Location of the content to edit"
                    },
                    "original": {
                        "type": "string",
                        "description": "Original content to replace"
                    },
                    "replacement": {
                        "type": "string",
                        "description": "New content"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for the edit"
                    }
                },
                "required": ["location", "original", "replacement"]
            }),
        )
        .with_confirmation()
    }

    /// Creates the insert tool definition.
    pub fn insert() -> ToolDefinition {
        ToolDefinition::new(
            "insert",
            "Insert new content into the document",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "object",
                        "properties": {
                            "page": { "type": "integer" },
                            "section": { "type": "string" },
                            "position": {
                                "type": "string",
                                "enum": ["before", "after", "start", "end"]
                            }
                        },
                        "description": "Where to insert the content"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to insert"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for the insertion"
                    }
                },
                "required": ["location", "content"]
            }),
        )
        .with_confirmation()
    }

    /// Creates the redact tool definition.
    pub fn redact() -> ToolDefinition {
        ToolDefinition::new(
            "redact",
            "Redact sensitive information from the document",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Pattern to match for redaction (regex supported)"
                    },
                    "category": {
                        "type": "string",
                        "enum": ["pii", "financial", "medical", "legal", "custom"],
                        "description": "Category of information to redact"
                    },
                    "replacement": {
                        "type": "string",
                        "description": "Replacement text (default: [REDACTED])"
                    },
                    "preview": {
                        "type": "boolean",
                        "description": "If true, return matches without redacting",
                        "default": false
                    }
                },
                "required": ["category"]
            }),
        )
        .with_confirmation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_builder_search() {
        let tool = ToolBuilder::search();
        assert_eq!(tool.name(), "search");
        assert!(tool.is_idempotent());
        assert!(!tool.requires_confirmation());
    }

    #[test]
    fn tool_builder_edit() {
        let tool = ToolBuilder::edit();
        assert_eq!(tool.name(), "edit");
        assert!(!tool.is_idempotent());
        assert!(tool.requires_confirmation());
    }

    #[test]
    fn tool_to_openai_format() {
        let tool = ToolBuilder::search();
        let openai = tool.to_openai_function();

        assert_eq!(openai["type"], "function");
        assert_eq!(openai["function"]["name"], "search");
    }
}
