//! Chat context management for maintaining conversation history.
//!
//! This module provides the [`ChatContext`] type for managing multi-turn conversations
//! with automatic message history and token usage tracking.

use portkey_sdk::model::{ChatCompletionRequestMessage, Usage};
use serde::{Deserialize, Serialize};

/// Context for a multi-turn chat conversation.
///
/// `ChatContext` maintains the message history and cumulative token usage for a conversation,
/// making it easy to build multi-turn interactions without manually tracking state.
///
/// # Examples
///
/// ```rust
/// use nvisy_portkey::completion::ChatContext;
///
/// // Create a new context with a system prompt
/// let mut context = ChatContext::new("You are a helpful assistant.");
///
/// // Add user messages
/// context.add_user_message("What is 2+2?");
///
/// // Get all messages
/// let messages = context.messages();
/// assert_eq!(messages.len(), 2); // system + user
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatContext {
    /// The message history for this conversation
    messages: Vec<ChatCompletionRequestMessage>,

    /// Cumulative token usage across all completions in this context
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Usage>,
}

impl ChatContext {
    /// Creates a new chat context with an optional system prompt.
    ///
    /// # Arguments
    ///
    /// * `system_prompt` - The initial system prompt for the conversation
    pub fn new(system_prompt: impl Into<String>) -> Self {
        let system_prompt = system_prompt.into();
        let messages = if system_prompt.is_empty() {
            Vec::new()
        } else {
            vec![ChatCompletionRequestMessage::System {
                content: system_prompt,
                name: None,
            }]
        };

        Self {
            messages,
            usage: None,
        }
    }

    /// Creates a new empty chat context without a system prompt.
    pub fn empty() -> Self {
        Self {
            messages: Vec::new(),
            usage: None,
        }
    }

    /// Creates a chat context from existing messages.
    ///
    /// # Arguments
    ///
    /// * `messages` - The initial message history
    pub fn from_messages(messages: Vec<ChatCompletionRequestMessage>) -> Self {
        Self {
            messages,
            usage: None,
        }
    }

    /// Adds a user message to the conversation.
    ///
    /// # Arguments
    ///
    /// * `content` - The content of the user message
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages
            .push(ChatCompletionRequestMessage::user(content.into()));
    }

    /// Adds an assistant message to the conversation.
    ///
    /// # Arguments
    ///
    /// * `content` - The content of the assistant message
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages
            .push(ChatCompletionRequestMessage::assistant(content.into()));
    }

    /// Adds a system message to the conversation.
    ///
    /// # Arguments
    ///
    /// * `content` - The content of the system message
    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.messages.push(ChatCompletionRequestMessage::System {
            content: content.into(),
            name: None,
        });
    }

    /// Adds a custom message to the conversation.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to add
    pub fn add_message(&mut self, message: ChatCompletionRequestMessage) {
        self.messages.push(message);
    }

    /// Returns a reference to all messages in the conversation.
    pub fn messages(&self) -> &[ChatCompletionRequestMessage] {
        &self.messages
    }

    /// Returns a mutable reference to all messages in the conversation.
    pub fn messages_mut(&mut self) -> &mut Vec<ChatCompletionRequestMessage> {
        &mut self.messages
    }

    /// Returns the number of messages in the conversation.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns whether the conversation is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clears all messages from the conversation and resets usage.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.usage = None;
    }

    /// Clones the messages and returns them as a Vec.
    pub fn to_messages(&self) -> Vec<ChatCompletionRequestMessage> {
        self.messages.clone()
    }

    /// Returns the cumulative token usage for this conversation.
    pub fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// Returns the total number of prompt tokens used across all completions.
    pub fn total_prompt_tokens(&self) -> i32 {
        self.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0)
    }

    /// Returns the total number of completion tokens used across all completions.
    pub fn total_completion_tokens(&self) -> i32 {
        self.usage
            .as_ref()
            .map(|u| u.completion_tokens)
            .unwrap_or(0)
    }

    /// Returns the total number of tokens used across all completions.
    pub fn total_tokens(&self) -> i32 {
        self.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0)
    }

    /// Updates the cumulative usage with new completion usage.
    ///
    /// This is called internally by the completion methods.
    #[doc(hidden)]
    pub fn update_usage(&mut self, new_usage: Usage) {
        if let Some(existing) = &mut self.usage {
            existing.prompt_tokens += new_usage.prompt_tokens;
            existing.completion_tokens += new_usage.completion_tokens;
            existing.total_tokens += new_usage.total_tokens;
        } else {
            self.usage = Some(new_usage);
        }
    }
}

impl Default for ChatContext {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<Vec<ChatCompletionRequestMessage>> for ChatContext {
    fn from(messages: Vec<ChatCompletionRequestMessage>) -> Self {
        Self::from_messages(messages)
    }
}

impl From<ChatContext> for Vec<ChatCompletionRequestMessage> {
    fn from(context: ChatContext) -> Self {
        context.messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_system_prompt() {
        let context = ChatContext::new("You are helpful");
        assert_eq!(context.len(), 1);
        assert!(!context.is_empty());
        assert_eq!(context.total_tokens(), 0);
    }

    #[test]
    fn test_new_with_empty_system_prompt() {
        let context = ChatContext::new("");
        assert_eq!(context.len(), 0);
        assert!(context.is_empty());
    }

    #[test]
    fn test_empty() {
        let context = ChatContext::empty();
        assert_eq!(context.len(), 0);
        assert!(context.is_empty());
    }

    #[test]
    fn test_add_user_message() {
        let mut context = ChatContext::empty();
        context.add_user_message("Hello");
        assert_eq!(context.len(), 1);
    }

    #[test]
    fn test_add_assistant_message() {
        let mut context = ChatContext::empty();
        context.add_assistant_message("Hi there!");
        assert_eq!(context.len(), 1);
    }

    #[test]
    fn test_conversation_flow() {
        let mut context = ChatContext::new("You are a math tutor");
        assert_eq!(context.len(), 1);

        context.add_user_message("What is 2+2?");
        assert_eq!(context.len(), 2);

        context.add_assistant_message("2+2 equals 4.");
        assert_eq!(context.len(), 3);
    }

    #[test]
    fn test_usage_tracking() {
        let mut context = ChatContext::empty();
        assert!(context.usage().is_none());
        assert_eq!(context.total_tokens(), 0);

        // Simulate usage update
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        context.update_usage(usage);

        assert_eq!(context.total_prompt_tokens(), 10);
        assert_eq!(context.total_completion_tokens(), 20);
        assert_eq!(context.total_tokens(), 30);

        // Add more usage
        let usage2 = Usage {
            prompt_tokens: 5,
            completion_tokens: 15,
            total_tokens: 20,
        };
        context.update_usage(usage2);

        assert_eq!(context.total_prompt_tokens(), 15);
        assert_eq!(context.total_completion_tokens(), 35);
        assert_eq!(context.total_tokens(), 50);
    }

    #[test]
    fn test_clear() {
        let mut context = ChatContext::new("System");
        context.add_user_message("Hello");

        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        context.update_usage(usage);

        assert_eq!(context.len(), 2);
        assert_eq!(context.total_tokens(), 30);

        context.clear();
        assert!(context.is_empty());
        assert_eq!(context.total_tokens(), 0);
    }

    #[test]
    fn test_default() {
        let context = ChatContext::default();
        assert!(context.is_empty());
    }
}
