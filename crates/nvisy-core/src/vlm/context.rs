//! Context management for VLM (Vision Language Model) operations.
//!
//! This module provides types for managing multimodal conversation sessions,
//! including message history, processing options, usage tracking, and metadata.
//!
//! The `Context` type serves as a stateful container for VLM interactions,
//! maintaining conversation state and tracking resource usage across multiple
//! request-response cycles.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Context information for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier for this context session.
    pub session_id: Uuid,
    /// User identifier associated with this context.
    pub user_id: Uuid,
    /// Conversation identifier for tracking related interactions.
    pub conversation_id: Uuid,
    /// Processing options and configuration.
    pub processing_options: ProcessingOptions,
    /// Messages in the conversation history.
    pub messages: Vec<Message>,
    /// Usage statistics for this context.
    pub usage: UsageStats,
    /// Metadata about the context and processing.
    pub metadata: ContextMetadata,
}

impl Context {
    /// Create a new VLM context.
    pub fn new(user_id: Uuid, conversation_id: Uuid) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            user_id,
            conversation_id,
            processing_options: ProcessingOptions::default(),
            messages: Vec::new(),
            usage: UsageStats::default(),
            metadata: ContextMetadata::default(),
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: Message) {
        // Update usage statistics
        self.usage.total_messages += 1;
        self.usage.total_tokens += message.token_count.unwrap_or(0);

        if message.role == MessageRole::User {
            self.usage.user_messages += 1;
        } else if message.role == MessageRole::Assistant {
            self.usage.assistant_messages += 1;
            if !message.content.trim().is_empty() {
                self.usage.successful_responses += 1;
            } else {
                self.usage.failed_responses += 1;
            }
        }

        self.messages.push(message);
    }

    /// Get all messages in the conversation.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get messages of a specific role.
    pub fn messages_by_role(&self, role: MessageRole) -> Vec<&Message> {
        self.messages.iter().filter(|m| m.role == role).collect()
    }

    /// Get the last message in the conversation.
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get the last user message.
    pub fn last_user_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
    }

    /// Get the last assistant message.
    pub fn last_assistant_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
    }

    /// Get the number of messages in the conversation.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if the conversation has any messages.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Clear all messages from the conversation.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.usage = UsageStats::default();
    }
}

/// Processing options for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOptions {
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Temperature for response generation (0.0 to 1.0).
    pub temperature: Option<f32>,
    /// Whether to enable streaming responses.
    pub streaming: bool,
    /// Custom parameters for specific VLM engines.
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(1024),
            temperature: Some(0.7),
            streaming: false,
            custom_parameters: HashMap::new(),
        }
    }
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: Uuid,
    /// Role of the message sender.
    pub role: MessageRole,
    /// Text content of the message.
    pub content: String,
    /// Optional images associated with this message.
    pub images: Vec<ImageData>,
    /// Token count for this message.
    pub token_count: Option<u32>,
    /// When this message was created.
    pub created_at: Timestamp,
    /// Processing metadata for this message.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a new user message.
    pub fn user(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content,
            images: Vec::new(),
            token_count: None,
            created_at: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new user message with images.
    pub fn user_with_images(content: String, images: Vec<ImageData>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content,
            images,
            token_count: None,
            created_at: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content,
            images: Vec::new(),
            token_count: None,
            created_at: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new system message.
    pub fn system(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::System,
            content,
            images: Vec::new(),
            token_count: None,
            created_at: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Set token count for this message.
    pub fn with_token_count(mut self, count: u32) -> Self {
        self.token_count = Some(count);
        self
    }

    /// Add metadata to this message.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if this message has images.
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }

    /// Get the number of images in this message.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Check if this message is empty.
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty() && self.images.is_empty()
    }
}

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message from the user.
    User,
    /// Message from the assistant.
    Assistant,
    /// System message for context.
    System,
}

/// Image data associated with a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// Unique identifier for this image.
    pub id: Uuid,
    /// Image data as base64 encoded string.
    pub data: String,
    /// MIME type of the image.
    pub mime_type: String,
    /// Optional filename or description.
    pub filename: Option<String>,
    /// Image dimensions if known.
    pub dimensions: Option<(u32, u32)>,
    /// Size of the image in bytes.
    pub size_bytes: Option<u64>,
}

impl ImageData {
    /// Create new image data.
    pub fn new(data: String, mime_type: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
            mime_type,
            filename: None,
            dimensions: None,
            size_bytes: None,
        }
    }

    /// Set filename for this image.
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Set dimensions for this image.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.dimensions = Some((width, height));
        self
    }

    /// Set size for this image.
    pub fn with_size(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }
}

/// Usage statistics for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct UsageStats {
    /// Total number of messages in the conversation.
    pub total_messages: usize,
    /// Total number of user messages.
    pub user_messages: usize,
    /// Total number of assistant messages.
    pub assistant_messages: usize,
    /// Total tokens used in the conversation.
    pub total_tokens: u32,
    /// Number of successful responses.
    pub successful_responses: usize,
    /// Number of failed responses.
    pub failed_responses: usize,
    /// Total processing time in milliseconds.
    pub total_processing_time_ms: u64,
    /// Estimated cost for this conversation.
    pub estimated_cost: Option<f64>,
}


impl UsageStats {
    /// Get total number of responses.
    pub fn total_responses(&self) -> usize {
        self.successful_responses + self.failed_responses
    }

    /// Get success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        let total = self.total_responses();
        if total == 0 {
            100.0
        } else {
            (self.successful_responses as f32 / total as f32) * 100.0
        }
    }

    /// Get average processing time per response.
    pub fn average_processing_time_per_response(&self) -> Option<f32> {
        let total = self.total_responses();
        if total == 0 {
            None
        } else {
            Some(self.total_processing_time_ms as f32 / total as f32)
        }
    }

    /// Get average tokens per message.
    pub fn average_tokens_per_message(&self) -> f32 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.total_tokens as f32 / self.total_messages as f32
        }
    }

    /// Check if there's any usage data.
    pub fn has_usage(&self) -> bool {
        self.total_messages > 0
    }
}

/// Context metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// When the context was created.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub last_updated: Timestamp,
    /// VLM model used.
    pub model: Option<String>,
    /// Model version.
    pub model_version: Option<String>,
    /// Processing mode.
    pub processing_mode: Option<String>,
    /// Custom tags.
    pub tags: Vec<String>,
}

impl Default for ContextMetadata {
    fn default() -> Self {
        let now = Timestamp::now();
        Self {
            created_at: now,
            last_updated: now,
            model: None,
            model_version: None,
            processing_mode: None,
            tags: Vec::new(),
        }
    }
}
