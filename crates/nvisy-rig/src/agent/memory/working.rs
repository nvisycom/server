//! Working memory for agent context management.
//!
//! Provides a key-value store for agent working context that persists
//! across turns within a conversation.

use std::collections::HashMap;

/// Working memory for storing agent context between turns.
///
/// This provides a simple key-value store for agents to maintain
/// context information like extracted entities, intermediate results,
/// or user preferences during a conversation.
#[derive(Debug, Clone, Default)]
pub struct WorkingMemory {
    /// Key-value storage for context data.
    entries: HashMap<String, String>,

    /// Maximum number of entries to store.
    capacity: usize,
}

impl WorkingMemory {
    /// Creates a new working memory with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// Stores a value in working memory.
    ///
    /// If the key already exists, the value is updated.
    /// If capacity is exceeded, the oldest entry is removed.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        use std::collections::hash_map::Entry;

        let key = key.into();
        let value = value.into();

        // Check if we need to make room before borrowing via entry()
        let needs_eviction =
            !self.entries.contains_key(&key) && self.entries.len() >= self.capacity;

        if needs_eviction && let Some(remove_key) = self.entries.keys().next().cloned() {
            self.entries.remove(&remove_key);
        }

        match self.entries.entry(key) {
            Entry::Occupied(mut e) => {
                e.insert(value);
            }
            Entry::Vacant(e) => {
                e.insert(value);
            }
        }
    }

    /// Retrieves a value from working memory.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    /// Removes a value from working memory.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.entries.remove(key)
    }

    /// Checks if a key exists in working memory.
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns all keys in working memory.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// Returns the number of entries in working memory.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if working memory is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all entries from working memory.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Formats working memory as a context string for prompts.
    pub fn to_context_string(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let mut context = String::from("Working Memory:\n");
        for (key, value) in &self.entries {
            context.push_str(&format!("- {}: {}\n", key, value));
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_memory_is_empty() {
        let memory = WorkingMemory::new(10);
        assert!(memory.is_empty());
        assert_eq!(memory.len(), 0);
    }

    #[test]
    fn set_and_get() {
        let mut memory = WorkingMemory::new(10);
        memory.set("user_name", "Alice");

        assert_eq!(memory.get("user_name"), Some("Alice"));
    }

    #[test]
    fn update_existing_key() {
        let mut memory = WorkingMemory::new(10);
        memory.set("count", "1");
        memory.set("count", "2");

        assert_eq!(memory.get("count"), Some("2"));
        assert_eq!(memory.len(), 1);
    }

    #[test]
    fn remove_entry() {
        let mut memory = WorkingMemory::new(10);
        memory.set("key", "value");

        let removed = memory.remove("key");
        assert_eq!(removed, Some("value".to_string()));
        assert!(memory.is_empty());
    }

    #[test]
    fn respects_capacity() {
        let mut memory = WorkingMemory::new(2);
        memory.set("a", "1");
        memory.set("b", "2");
        memory.set("c", "3");

        assert_eq!(memory.len(), 2);
    }

    #[test]
    fn context_string_format() {
        let mut memory = WorkingMemory::new(10);
        memory.set("task", "summarize");

        let context = memory.to_context_string();
        assert!(context.contains("Working Memory:"));
        assert!(context.contains("task: summarize"));
    }

    #[test]
    fn empty_context_string() {
        let memory = WorkingMemory::new(10);
        assert!(memory.to_context_string().is_empty());
    }
}
