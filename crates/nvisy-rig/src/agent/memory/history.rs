//! Chat history with automatic compaction strategies.
//!
//! Provides conversation history management with configurable compaction
//! strategies to handle context window limits.

use rig::message::Message;

/// Strategy for compacting chat history when capacity is exceeded.
#[derive(Debug, Clone, Default)]
pub enum CompactionStrategy {
    /// Truncate oldest messages, keeping the most recent ones.
    #[default]
    Truncate,

    /// Summarize older messages into a context string.
    Summarize {
        /// Summary of compacted messages.
        summary: String,
    },
}

/// Chat history with automatic compaction.
///
/// Manages conversation history with a configurable capacity limit and
/// compaction strategy for handling context window constraints.
#[derive(Debug, Clone)]
pub struct ChatHistory {
    /// Messages in the conversation.
    messages: Vec<Message>,

    /// Maximum number of messages before compaction.
    capacity: usize,

    /// Strategy for handling overflow.
    strategy: CompactionStrategy,
}

impl ChatHistory {
    /// Creates a new chat history with the given capacity.
    ///
    /// Uses truncation as the default compaction strategy.
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
            capacity,
            strategy: CompactionStrategy::Truncate,
        }
    }

    /// Creates a chat history with a custom compaction strategy.
    pub fn with_strategy(capacity: usize, strategy: CompactionStrategy) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
            capacity,
            strategy,
        }
    }

    /// Adds a message to the history, compacting if necessary.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);

        if self.messages.len() > self.capacity {
            self.compact();
        }
    }

    /// Adds multiple messages to the history.
    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            self.push(message);
        }
    }

    /// Returns the current messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Returns the number of messages currently stored.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns true if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clears all messages and resets the summary.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.strategy = CompactionStrategy::Truncate;
    }

    /// Sets a new compaction strategy.
    pub fn set_strategy(&mut self, strategy: CompactionStrategy) {
        self.strategy = strategy;
    }

    /// Updates the summary for summarize strategy.
    ///
    /// This should be called with an LLM-generated summary of the
    /// compacted messages.
    pub fn set_summary(&mut self, summary: String) {
        self.strategy = CompactionStrategy::Summarize { summary };
    }

    /// Returns the current summary if using summarize strategy.
    pub fn summary(&self) -> Option<&str> {
        match &self.strategy {
            CompactionStrategy::Summarize { summary } => Some(summary),
            CompactionStrategy::Truncate => None,
        }
    }

    /// Compacts the history according to the current strategy.
    fn compact(&mut self) {
        let keep_count = self.capacity / 2;
        let remove_count = self.messages.len().saturating_sub(keep_count);

        if remove_count == 0 {
            return;
        }

        match &mut self.strategy {
            CompactionStrategy::Truncate => {
                // Simply remove oldest messages
                self.messages.drain(0..remove_count);
            }
            CompactionStrategy::Summarize { .. } => {
                // Remove oldest messages (caller should update summary separately)
                self.messages.drain(0..remove_count);
            }
        }
    }
}

impl Default for ChatHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_history_is_empty() {
        let history = ChatHistory::new(10);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn push_adds_messages() {
        let mut history = ChatHistory::new(10);
        history.push(Message::user("Hello"));
        history.push(Message::assistant("Hi!"));

        assert_eq!(history.len(), 2);
    }

    #[test]
    fn truncate_compacts_when_over_capacity() {
        let mut history = ChatHistory::new(4);

        for i in 0..6 {
            history.push(Message::user(format!("Message {}", i)));
        }

        // Should have compacted, keeping capacity/2 = 2 messages
        assert!(history.len() <= 4);
    }

    #[test]
    fn summarize_strategy_stores_summary() {
        let mut history = ChatHistory::with_strategy(
            10,
            CompactionStrategy::Summarize {
                summary: String::new(),
            },
        );

        history.push(Message::user("Hello"));
        history.set_summary("User greeted the assistant.".to_string());

        assert_eq!(history.summary(), Some("User greeted the assistant."));
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn clear_resets_history() {
        let mut history = ChatHistory::new(10);
        history.push(Message::user("Test"));
        history.set_summary("Summary".to_string());

        history.clear();

        assert!(history.is_empty());
        assert!(history.summary().is_none());
    }
}
