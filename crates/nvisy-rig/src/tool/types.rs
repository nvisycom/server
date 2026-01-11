//! Tool input/output types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Input to a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    /// The call ID.
    pub call_id: Uuid,

    /// Arguments from the tool call.
    pub arguments: Value,
}

impl ToolInput {
    /// Gets a string argument.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.arguments.get(key).and_then(|v| v.as_str())
    }

    /// Gets an integer argument.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.arguments.get(key).and_then(|v| v.as_i64())
    }

    /// Gets a boolean argument.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.arguments.get(key).and_then(|v| v.as_bool())
    }

    /// Gets an object argument.
    pub fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, Value>> {
        self.arguments.get(key).and_then(|v| v.as_object())
    }

    /// Gets an array argument.
    pub fn get_array(&self, key: &str) -> Option<&Vec<Value>> {
        self.arguments.get(key).and_then(|v| v.as_array())
    }

    /// Deserializes the arguments to a typed struct.
    pub fn parse<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.arguments.clone())
    }
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolOutput {
    /// Plain text output.
    Text { content: String },

    /// JSON output.
    Json { data: Value },

    /// Binary data (base64 encoded).
    Binary { data: String, mime_type: String },

    /// Proposed edit output.
    Edit {
        edit_id: Uuid,
        description: String,
        preview: Option<String>,
    },

    /// Multiple outputs.
    Multiple { outputs: Vec<ToolOutput> },

    /// Empty output.
    Empty,
}

impl ToolOutput {
    /// Creates a text output.
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
        }
    }

    /// Creates a JSON output.
    pub fn json(data: Value) -> Self {
        Self::Json { data }
    }

    /// Creates a binary output.
    pub fn binary(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Binary {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Creates an edit output.
    pub fn edit(edit_id: Uuid, description: impl Into<String>) -> Self {
        Self::Edit {
            edit_id,
            description: description.into(),
            preview: None,
        }
    }

    /// Creates an edit output with preview.
    pub fn edit_with_preview(
        edit_id: Uuid,
        description: impl Into<String>,
        preview: impl Into<String>,
    ) -> Self {
        Self::Edit {
            edit_id,
            description: description.into(),
            preview: Some(preview.into()),
        }
    }

    /// Creates a multiple output.
    pub fn multiple(outputs: Vec<ToolOutput>) -> Self {
        Self::Multiple { outputs }
    }

    /// Creates an empty output.
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Converts to a string representation for the LLM.
    pub fn to_llm_string(&self) -> String {
        match self {
            Self::Text { content } => content.clone(),
            Self::Json { data } => serde_json::to_string_pretty(data).unwrap_or_default(),
            Self::Binary { mime_type, .. } => format!("[Binary data: {mime_type}]"),
            Self::Edit {
                edit_id,
                description,
                preview,
            } => {
                if let Some(p) = preview {
                    format!("[Edit proposed: {edit_id}]\n{description}\n\nPreview:\n{p}")
                } else {
                    format!("[Edit proposed: {edit_id}]\n{description}")
                }
            }
            Self::Multiple { outputs } => outputs
                .iter()
                .map(|o| o.to_llm_string())
                .collect::<Vec<_>>()
                .join("\n---\n"),
            Self::Empty => "[No output]".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_input_get_string() {
        let input = ToolInput {
            call_id: Uuid::now_v7(),
            arguments: serde_json::json!({
                "query": "test",
                "count": 5
            }),
        };

        assert_eq!(input.get_string("query"), Some("test"));
        assert_eq!(input.get_i64("count"), Some(5));
        assert_eq!(input.get_string("missing"), None);
    }

    #[test]
    fn tool_output_to_llm_string() {
        let text = ToolOutput::text("hello");
        assert_eq!(text.to_llm_string(), "hello");

        let empty = ToolOutput::empty();
        assert_eq!(empty.to_llm_string(), "[No output]");
    }
}
