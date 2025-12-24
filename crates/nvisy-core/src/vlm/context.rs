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

use crate::types::{Annotation, Chat, Message, MessageRole};

/// Context information for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// User identifier associated with this context.
    pub user_id: Uuid,
    /// Processing options and configuration.
    pub processing_options: ProcessingOptions,
    /// Chat conversation history.
    pub chat: Chat,
    /// Usage statistics for this context.
    pub usage: UsageStats,
    /// Metadata about the context and processing.
    pub metadata: ContextMetadata,
    /// Annotations associated with this context.
    pub annotations: Vec<Annotation>,
}

impl Context {
    /// Create a new VLM context.
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            processing_options: ProcessingOptions::default(),
            chat: Chat::new(),
            usage: UsageStats::default(),
            metadata: ContextMetadata::default(),
            annotations: Vec::new(),
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: Message) {
        // Update usage statistics
        self.usage.total_messages += 1;
        // Token counting would need to be implemented based on message content

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

        self.chat.add_message(message);
    }

    /// Get all messages in the conversation.
    pub fn messages(&self) -> &[Message] {
        &self.chat.messages
    }

    /// Get messages of a specific role.
    pub fn messages_by_role(&self, role: MessageRole) -> Vec<&Message> {
        self.chat
            .messages
            .iter()
            .filter(|m| m.role == role)
            .collect()
    }

    /// Get the last message in the conversation.
    pub fn last_message(&self) -> Option<&Message> {
        self.chat.last_message()
    }

    /// Get the last user message.
    pub fn last_user_message(&self) -> Option<&Message> {
        self.chat
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
    }

    /// Get the last assistant message.
    pub fn last_assistant_message(&self) -> Option<&Message> {
        self.chat
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
    }

    /// Get the number of messages in the conversation.
    pub fn message_count(&self) -> usize {
        self.chat.message_count()
    }

    /// Check if the conversation has any messages.
    pub fn has_messages(&self) -> bool {
        self.chat.message_count() > 0
    }

    /// Clear all messages from the conversation.
    pub fn clear_messages(&mut self) {
        self.chat = Chat::new();
        self.usage = UsageStats::default();
        self.annotations.clear();
    }

    /// Sets the annotations for this context.
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = annotations;
        self
    }

    /// Adds an annotation to this context.
    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotations.push(annotation);
        self
    }

    /// Gets all annotations.
    pub fn get_annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Adds an annotation.
    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
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
