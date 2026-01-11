//! Chat message types.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System prompt.
    System,
    /// User message.
    User,
    /// Assistant response.
    Assistant,
    /// Tool result.
    Tool,
}

/// A message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID.
    id: Uuid,

    /// Message role.
    role: MessageRole,

    /// Message content.
    content: String,

    /// Tool call ID (for tool messages).
    tool_call_id: Option<Uuid>,

    /// When the message was created.
    created_at: Timestamp,
}

impl Message {
    /// Creates a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            created_at: Timestamp::now(),
        }
    }

    /// Creates a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            created_at: Timestamp::now(),
        }
    }

    /// Creates an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            created_at: Timestamp::now(),
        }
    }

    /// Creates a tool result message.
    pub fn tool(tool_call_id: Uuid, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id),
            created_at: Timestamp::now(),
        }
    }

    /// Returns the message ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the message role.
    pub fn role(&self) -> MessageRole {
        self.role
    }

    /// Returns the message content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the tool call ID if this is a tool message.
    pub fn tool_call_id(&self) -> Option<Uuid> {
        self.tool_call_id
    }

    /// Returns when the message was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_roles() {
        let system = Message::system("You are a helpful assistant");
        let user = Message::user("Hello");
        let assistant = Message::assistant("Hi!");
        let tool = Message::tool(Uuid::now_v7(), "result");

        assert_eq!(system.role(), MessageRole::System);
        assert_eq!(user.role(), MessageRole::User);
        assert_eq!(assistant.role(), MessageRole::Assistant);
        assert_eq!(tool.role(), MessageRole::Tool);
    }

    #[test]
    fn tool_message_has_call_id() {
        let call_id = Uuid::now_v7();
        let tool = Message::tool(call_id, "result");

        assert_eq!(tool.tool_call_id(), Some(call_id));
    }
}
