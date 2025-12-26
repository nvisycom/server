//! Conversation payload types and utilities.
//!
//! This module provides payload structures and utilities for conversation collections,
//! including point definitions, message types, and conversation-specific metadata.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SearchResult;
use crate::error::{Error, Result};
use crate::types::{Payload, Point, PointId, Vector};

/// Create a payload with standard metadata fields
fn create_metadata_payload() -> Payload {
    let now = jiff::Timestamp::now().to_string();
    Payload::new()
        .with("created_at", now.clone())
        .with("updated_at", now)
        .with("version", 1)
}

/// Types of messages in conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum MessageType {
    /// User message
    User,
    /// Assistant/AI response
    Assistant,
    /// System message
    System,
    /// Tool/function call message
    Tool,
    /// Media message (image, audio, video)
    Media,
    /// File attachment message
    File,
    /// Custom message type
    Custom(String),
}

impl MessageType {
    /// Get the string representation of the message type
    pub fn as_str(&self) -> &str {
        match self {
            MessageType::User => "user",
            MessageType::Assistant => "assistant",
            MessageType::System => "system",
            MessageType::Tool => "tool",
            MessageType::Media => "media",
            MessageType::File => "file",
            MessageType::Custom(name) => name,
        }
    }
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Conversation status for filtering and management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ConversationStatus {
    /// Active conversation
    Active,
    /// Archived conversation
    Archived,
    /// Deleted conversation
    Deleted,
    /// Draft conversation
    Draft,
    /// Paused conversation
    Paused,
}

impl ConversationStatus {
    /// Get the string representation of the conversation status
    pub fn as_str(&self) -> &str {
        match self {
            ConversationStatus::Active => "active",
            ConversationStatus::Archived => "archived",
            ConversationStatus::Deleted => "deleted",
            ConversationStatus::Draft => "draft",
            ConversationStatus::Paused => "paused",
        }
    }
}

impl std::fmt::Display for ConversationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A point representing a message or conversation element in the vector database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ConversationPoint {
    /// Unique identifier for the message
    pub id: PointId,

    /// Vector embedding of the message content
    pub embedding: Vector,

    /// The conversation/thread ID this message belongs to
    pub conversation_id: String,

    /// Type of message
    pub message_type: MessageType,

    /// The message content
    pub content: String,

    /// ID of the participant who sent the message
    pub participant_id: String,

    /// Role of the participant (user, assistant, system, etc.)
    pub participant_role: Option<String>,

    /// Sequence number within the conversation
    pub sequence_number: Option<u32>,

    /// ID of the message this is a reply to (for threading)
    pub reply_to: Option<String>,

    /// Conversation status
    pub status: Option<ConversationStatus>,

    /// Additional conversation metadata
    pub metadata: Payload,
}

impl ConversationPoint {
    /// Create a new conversation point
    pub fn new(
        id: impl Into<PointId>,
        embedding: Vector,
        conversation_id: String,
        message_type: MessageType,
        content: String,
        participant_id: String,
    ) -> Self {
        Self {
            id: id.into(),
            embedding,
            conversation_id,
            message_type,
            content,
            participant_id,
            participant_role: None,
            sequence_number: None,
            reply_to: None,
            status: Some(ConversationStatus::Active),
            metadata: create_metadata_payload(),
        }
    }

    /// Create a user message
    pub fn user_message(
        id: impl Into<PointId>,
        embedding: Vector,
        conversation_id: String,
        content: String,
        participant_id: String,
    ) -> Self {
        let mut point = Self::new(
            id,
            embedding,
            conversation_id,
            MessageType::User,
            content,
            participant_id,
        );
        point.participant_role = Some("user".to_string());
        point
    }

    /// Create an assistant message
    pub fn assistant_message(
        id: impl Into<PointId>,
        embedding: Vector,
        conversation_id: String,
        content: String,
        assistant_id: String,
    ) -> Self {
        let mut point = Self::new(
            id,
            embedding,
            conversation_id,
            MessageType::Assistant,
            content,
            assistant_id,
        );
        point.participant_role = Some("assistant".to_string());
        point
    }

    /// Create a system message
    pub fn system_message(
        id: impl Into<PointId>,
        embedding: Vector,
        conversation_id: String,
        content: String,
    ) -> Self {
        let mut point = Self::new(
            id,
            embedding,
            conversation_id,
            MessageType::System,
            content,
            "system".to_string(),
        );
        point.participant_role = Some("system".to_string());
        point
    }

    /// Set the sequence number for this message
    pub fn with_sequence_number(mut self, sequence: u32) -> Self {
        self.sequence_number = Some(sequence);
        self
    }

    /// Set this message as a reply to another message
    pub fn with_reply_to(mut self, reply_to_id: String) -> Self {
        self.reply_to = Some(reply_to_id);
        self
    }

    /// Set the conversation status
    pub fn with_status(mut self, status: ConversationStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set the participant role
    pub fn with_participant_role(mut self, role: String) -> Self {
        self.participant_role = Some(role);
        self
    }

    /// Add additional metadata
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Convert to a generic Point for storage
    pub fn to_point(self) -> Point {
        let mut payload = Payload::new()
            .with("conversation_id", self.conversation_id)
            .with("message_type", self.message_type.as_str())
            .with("content", self.content)
            .with("participant_id", self.participant_id);

        if let Some(role) = self.participant_role {
            payload = payload.with("participant_role", role);
        }

        if let Some(sequence) = self.sequence_number {
            payload = payload.with("sequence_number", sequence as i64);
        }

        if let Some(reply_to) = self.reply_to {
            payload = payload.with("reply_to", reply_to);
        }

        if let Some(status) = self.status {
            payload = payload.with("status", status.as_str());
        }

        // Merge additional metadata
        payload.merge(&self.metadata);

        Point::new(self.id, self.embedding, payload)
    }

    /// Create from a search result
    pub fn from_search_result(result: SearchResult) -> Result<Self> {
        let id = result.id.clone();
        let embedding = result.vector().unwrap_or_default();
        let payload = result.payload;

        let conversation_id = payload
            .get_string("conversation_id")
            .ok_or_else(|| Error::invalid_input().with_message("Missing conversation_id"))?
            .to_string();

        let message_type = match payload.get_string("message_type") {
            Some(type_str) => match type_str {
                "user" => MessageType::User,
                "assistant" => MessageType::Assistant,
                "system" => MessageType::System,
                "tool" => MessageType::Tool,
                "media" => MessageType::Media,
                "file" => MessageType::File,
                custom => MessageType::Custom(custom.to_string()),
            },
            None => {
                return Err(Error::invalid_input().with_message("Missing message_type"));
            }
        };

        let content = payload
            .get_string("content")
            .ok_or_else(|| Error::invalid_input().with_message("Missing content"))?
            .to_string();

        let participant_id = payload
            .get_string("participant_id")
            .ok_or_else(|| Error::invalid_input().with_message("Missing participant_id"))?
            .to_string();

        let participant_role = payload
            .get_string("participant_role")
            .map(|s| s.to_string());

        let sequence_number = payload.get_i64("sequence_number").map(|n| n as u32);

        let reply_to = payload.get_string("reply_to").map(|s| s.to_string());

        let status = payload.get_string("status").map(|s| match s {
            "active" => ConversationStatus::Active,
            "archived" => ConversationStatus::Archived,
            "deleted" => ConversationStatus::Deleted,
            "draft" => ConversationStatus::Draft,
            "paused" => ConversationStatus::Paused,
            _ => ConversationStatus::Active,
        });

        Ok(Self {
            id,
            embedding,
            conversation_id,
            message_type,
            content,
            participant_id,
            participant_role,
            sequence_number,
            reply_to,
            status,
            metadata: payload,
        })
    }
}

impl From<ConversationPoint> for Point {
    fn from(conversation: ConversationPoint) -> Self {
        conversation.to_point()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::User.as_str(), "user");
        assert_eq!(MessageType::Assistant.as_str(), "assistant");
        assert_eq!(MessageType::Custom("test".to_string()).as_str(), "test");
    }

    #[test]
    fn test_conversation_status_conversion() {
        assert_eq!(ConversationStatus::Active.as_str(), "active");
        assert_eq!(ConversationStatus::Archived.as_str(), "archived");
    }

    #[test]
    fn test_conversation_point_creation() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let point = ConversationPoint::user_message(
            "msg-123",
            vector,
            "conv-456".to_string(),
            "Hello world".to_string(),
            "user-789".to_string(),
        );

        assert_eq!(point.message_type, MessageType::User);
        assert_eq!(point.content, "Hello world");
        assert_eq!(point.conversation_id, "conv-456");
        assert_eq!(point.participant_id, "user-789");
        assert_eq!(point.participant_role, Some("user".to_string()));
    }

    #[test]
    fn test_conversation_point_with_metadata() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let point = ConversationPoint::assistant_message(
            "msg-123",
            vector,
            "conv-456".to_string(),
            "AI response".to_string(),
            "ai-assistant".to_string(),
        )
        .with_sequence_number(5)
        .with_reply_to("msg-122".to_string())
        .with_status(ConversationStatus::Active);

        assert_eq!(point.sequence_number, Some(5));
        assert_eq!(point.reply_to, Some("msg-122".to_string()));
        assert_eq!(point.status, Some(ConversationStatus::Active));
    }

    #[test]
    fn test_conversation_point_to_point_conversion() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let conversation_point = ConversationPoint::system_message(
            "msg-123",
            vector,
            "conv-456".to_string(),
            "System notification".to_string(),
        )
        .with_sequence_number(1);

        let point = conversation_point.to_point();

        assert_eq!(point.payload.get_string("message_type"), Some("system"));
        assert_eq!(
            point.payload.get_string("content"),
            Some("System notification")
        );
        assert_eq!(
            point.payload.get_string("conversation_id"),
            Some("conv-456")
        );
        assert_eq!(point.payload.get_i64("sequence_number"), Some(1));
    }
}
