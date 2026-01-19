//! Chat events emitted during streaming.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ChatResponse;
use crate::tool::edit::ProposedEdit;
use crate::tool::{ToolCall, ToolResult};

/// Events emitted during chat processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// Agent is thinking/planning.
    Thinking { content: String },

    /// Text delta from the model.
    TextDelta { delta: String },

    /// Agent is calling a tool.
    ToolCall { call: ToolCall },

    /// Tool execution completed.
    ToolResult { result: ToolResult },

    /// Agent proposes an edit to the document.
    ProposedEdit { edit: ProposedEdit },

    /// Edit was auto-applied based on policy.
    EditApplied { edit_id: Uuid },

    /// Chat response completed.
    Done { response: ChatResponse },

    /// Error occurred during processing.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_event_serialization() {
        let event = ChatEvent::TextDelta {
            delta: "Hello".to_string(),
        };

        let json = serde_json::to_string(&event).expect("ChatEvent should serialize to JSON");
        assert!(json.contains("text_delta"));
        assert!(json.contains("Hello"));
    }
}
