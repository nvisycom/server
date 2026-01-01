//! Message and chat conversation types for AI interactions.
//!
//! This module provides structures for representing conversational AI interactions,
//! including individual messages, chat histories, and conversation metadata.
//! Messages support multimodal content including text, images, and other data types.

use std::collections::HashMap;
use std::fmt;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Content;
use crate::{Error, Result};

/// A message in a conversational AI interaction.
///
/// Messages can contain various types of content including text, images, and binary data.
/// They support role-based conversations (user, assistant, system) and rich metadata
/// for tracking conversation context and processing information.
///
/// # Examples
///
/// User text message:
/// ```rust
/// use nvisy_service::{Message, MessageRole};
///
/// let message = Message::new(MessageRole::User, "What is the capital of France?");
/// ```
///
/// Assistant response with metadata:
/// ```rust
/// use nvisy_service::{Message, MessageRole};
///
/// let response = Message::new(MessageRole::Assistant, "The capital of France is Paris.")
///     .with_model("gpt-4")
///     .with_token_count(15);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: Uuid,

    /// Role of the message sender.
    pub role: MessageRole,

    /// Text content of the message.
    pub content: String,

    /// Additional content parts (images, files, etc.).
    pub content_parts: Vec<Content>,

    /// Name or identifier of the message sender.
    pub name: Option<String>,

    /// Model that generated this message (for assistant messages).
    pub model: Option<String>,

    /// Token count for this message.
    pub token_count: Option<u32>,

    /// Timestamp when this message was created.
    pub created_at: Timestamp,

    /// Processing duration for this message.
    pub processing_time: Option<std::time::Duration>,

    /// Additional structured metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Role of a message participant in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message from a human user.
    User,

    /// Message from an AI assistant.
    Assistant,

    /// System message providing instructions or context.
    System,

    /// Message from a function or tool call.
    Function,

    /// Message from an external tool or service.
    Tool,
}

impl Message {
    /// Creates a new message with the specified role and content.
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            content_parts: Vec::new(),
            name: None,
            model: None,
            token_count: None,
            created_at: Timestamp::now(),
            processing_time: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the message name/identifier.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the model that generated this message.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the token count for this message.
    pub fn with_token_count(mut self, token_count: u32) -> Self {
        self.token_count = Some(token_count);
        self
    }

    /// Sets the processing time for this message.
    pub fn with_processing_time(mut self, processing_time: std::time::Duration) -> Self {
        self.processing_time = Some(processing_time);
        self
    }

    /// Adds a content part to this message.
    pub fn with_content_part(mut self, content: Content) -> Self {
        self.content_parts.push(content);
        self
    }

    /// Adds multiple content parts to this message.
    pub fn with_content_parts(mut self, content_parts: Vec<Content>) -> Self {
        self.content_parts.extend(content_parts);
        self
    }

    /// Adds metadata to this message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Returns the total estimated size of this message.
    pub fn estimated_size(&self) -> usize {
        self.content.len()
            + self
                .content_parts
                .iter()
                .map(|part| part.estimated_size())
                .sum::<usize>()
            + self.name.as_ref().map(|n| n.len()).unwrap_or(0)
            + self.model.as_ref().map(|m| m.len()).unwrap_or(0)
            + self.metadata.len() * 50 // Rough estimate for metadata
    }
}

/// A chat conversation consisting of a sequence of messages.
///
/// Chat represents a complete conversational context including all messages,
/// participant information, and conversation metadata. It provides methods
/// for managing message history and extracting conversation insights.
///
/// # Examples
///
/// Creating a new chat:
/// ```rust
/// use nvisy_service::{Chat, Message, MessageRole};
///
/// let mut chat = Chat::new();
///
/// let user_msg = Message::new(MessageRole::User, "Hello, how are you?");
/// let assistant_msg = Message::new(MessageRole::Assistant, "I'm doing well, thank you for asking!");
///
/// chat.add_message(user_msg);
/// chat.add_message(assistant_msg);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    /// Unique identifier for this chat.
    pub id: Uuid,

    /// Title or subject of the chat.
    pub title: Option<String>,

    /// All messages in chronological order.
    pub messages: Vec<Message>,

    /// Timestamp when the chat was created.
    pub created_at: Timestamp,

    /// Timestamp when the chat was last updated.
    pub updated_at: Timestamp,

    /// Chat-level metadata and settings.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Returns true if this is a user message.
    pub fn is_user_message(&self) -> bool {
        self.role == MessageRole::User
    }

    /// Returns true if this is an assistant message.
    pub fn is_assistant_message(&self) -> bool {
        self.role == MessageRole::Assistant
    }

    /// Returns true if this is a system message.
    pub fn is_system_message(&self) -> bool {
        self.role == MessageRole::System
    }

    /// Returns the total content length including all parts.
    pub fn content_length(&self) -> usize {
        self.estimated_size()
    }

    /// Checks if the message contains any documents.
    pub fn has_documents(&self) -> bool {
        self.content_parts.iter().any(|part| part.is_document())
    }

    /// Gets all document content parts from this message.
    pub fn get_documents(&self) -> Vec<&Content> {
        self.content_parts
            .iter()
            .filter(|part| part.is_document())
            .collect()
    }

    /// Validates the message content and metadata.
    pub fn validate(&self) -> Result<()> {
        if self.content.is_empty() && self.content_parts.is_empty() {
            return Err(
                Error::invalid_input().with_message("Message must have content or content parts")
            );
        }

        Ok(())
    }
}

impl Chat {
    /// Creates a new empty chat.
    pub fn new() -> Self {
        let now = Timestamp::now();
        Self {
            id: Uuid::new_v4(),
            title: None,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Sets the chat title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Adds a message to the chat.
    pub fn with_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self.updated_at = Timestamp::now();
        self
    }

    /// Adds metadata to the chat.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Adds a message to the chat.
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Timestamp::now();
    }

    /// Gets the last message in the chat.
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Gets messages by role.
    pub fn messages_by_role(&self, role: MessageRole) -> Vec<&Message> {
        self.messages.iter().filter(|m| m.role == role).collect()
    }

    /// Gets the total number of messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Gets the total token count for the conversation.
    pub fn total_tokens(&self) -> u32 {
        self.messages
            .iter()
            .map(|m| m.token_count.unwrap_or(0))
            .sum()
    }

    /// Estimates the total size of this chat in bytes.
    pub fn estimated_size(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.estimated_size())
            .sum::<usize>()
            + self.title.as_ref().map(|t| t.len()).unwrap_or(0)
            + self.metadata.len() * 50 // Rough estimate for metadata
    }

    /// Validates the chat structure.
    pub fn validate(&self) -> Result<()> {
        for message in &self.messages {
            message.validate()?;
        }
        Ok(())
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
            Self::Function => write!(f, "function"),
            Self::Tool => write!(f, "tool"),
        }
    }
}
