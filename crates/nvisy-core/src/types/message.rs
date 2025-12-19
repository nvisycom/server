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

use super::{Annotation, Document, Result, TypeError};

/// A message in a conversational AI interaction.
///
/// Messages can contain various types of content including text, images, and binary data.
/// They support role-based conversations (user, assistant, system) and rich metadata
/// for tracking conversation context and processing information.
///
/// # Examples
///
/// User text message:
/// ```rust,ignore
/// use nvisy_core::types::{Message, MessageRole};
///
/// let message = Message::builder()
///     .role(MessageRole::User)
///     .content("What is the capital of France?")
///     .build()?;
/// ```
///
/// Assistant response with metadata:
/// ```rust,ignore
/// let response = Message::builder()
///     .role(MessageRole::Assistant)
///     .content("The capital of France is Paris.")
///     .model("gpt-4")
///     .token_count(Some(15))
///     .build()?;
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
    pub content_parts: Vec<ContentPart>,

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

/// A part of message content that can contain various types of data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ContentPart {
    /// Plain text content.
    Text { text: String },

    /// Annotation content.
    Annotation {
        /// The annotation data.
        annotation: Annotation,
    },

    /// Document content.
    Document {
        /// The document data.
        document: Document,
    },
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
/// ```rust,ignore
/// use nvisy_core::types::{Chat, Message, MessageRole};
///
/// let mut chat = Chat::new();
///
/// let user_msg = Message::builder()
///     .role(MessageRole::User)
///     .content("Hello, how are you?")
///     .build()?;
///
/// let assistant_msg = Message::builder()
///     .role(MessageRole::Assistant)
///     .content("I'm doing well, thank you for asking!")
///     .build()?;
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
    /// Creates a new message with the given role and content.
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

    /// Sets the message ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the sender name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the model that generated this message.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the token count.
    pub fn with_token_count(mut self, count: u32) -> Self {
        self.token_count = Some(count);
        self
    }

    /// Adds a content part.
    pub fn with_content_part(mut self, part: ContentPart) -> Self {
        self.content_parts.push(part);
        self
    }

    /// Adds an annotation content part.
    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.content_parts
            .push(ContentPart::Annotation { annotation });
        self
    }

    /// Adds a document content part.
    pub fn with_document(mut self, document: Document) -> Self {
        self.content_parts.push(ContentPart::Document { document });
        self
    }

    /// Adds metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

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
        let text_len = self.content.len();
        let parts_len: usize = self
            .content_parts
            .iter()
            .map(|part| part.estimated_size())
            .sum();
        text_len + parts_len
    }

    /// Checks if the message contains any image content.
    pub fn has_images(&self) -> bool {
        false
    }

    /// Checks if the message contains any annotations.
    pub fn has_annotations(&self) -> bool {
        self.content_parts.iter().any(|part| part.is_annotation())
    }

    /// Checks if the message contains any documents.
    pub fn has_documents(&self) -> bool {
        self.content_parts.iter().any(|part| part.is_document())
    }

    /// Gets all image content parts.
    /// Gets all image content parts from this message.
    pub fn get_images(&self) -> Vec<&ContentPart> {
        Vec::new()
    }

    /// Gets all annotation content parts from this message.
    pub fn get_annotations(&self) -> Vec<&ContentPart> {
        self.content_parts
            .iter()
            .filter(|part| part.is_annotation())
            .collect()
    }

    /// Gets all document content parts from this message.
    pub fn get_documents(&self) -> Vec<&ContentPart> {
        self.content_parts
            .iter()
            .filter(|part| part.is_document())
            .collect()
    }

    /// Validates the message content and metadata.
    pub fn validate(&self) -> Result<()> {
        if self.content.is_empty() && self.content_parts.is_empty() {
            return Err(TypeError::ValidationFailed(
                "Message must have content or content parts".to_string(),
            ));
        }

        for part in &self.content_parts {
            part.validate()?;
        }

        Ok(())
    }
}

impl ContentPart {
    /// Returns true if this is an image content part.
    pub fn is_image(&self) -> bool {
        false
    }

    /// Returns true if this is an annotation content part.
    pub fn is_annotation(&self) -> bool {
        matches!(self, Self::Annotation { .. })
    }

    /// Returns true if this is a document content part.
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Document { .. })
    }

    /// Estimates the size of this content part for rate limiting.
    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Text { text } => text.len(),
            Self::Annotation { .. } => 256, // Estimated size for annotation metadata
            Self::Document { document } => document.content.len(),
        }
    }

    /// Validates the content part.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Text { text } => {
                if text.is_empty() {
                    return Err(TypeError::ValidationFailed(
                        "Text content cannot be empty".to_string(),
                    ));
                }
            }
            Self::Annotation { annotation } => {
                // Delegate validation to the annotation itself
                annotation.validate()?;
            }
            Self::Document { document } => {
                // Delegate validation to the document itself
                document.validate()?;
            }
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

    /// Calculates statistics for this chat.

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
