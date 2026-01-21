//! Execution context for workflow runs.

use std::collections::HashMap;

use derive_builder::Builder;
use nvisy_dal::AnyDataValue;

use crate::provider::CredentialsRegistry;

/// Execution context for a workflow run.
///
/// Manages the current data items flowing through the pipeline, holds
/// credentials for provider access, and provides named cache slots for
/// data sharing between workflow branches.
///
/// A single input can produce multiple outputs (e.g., 1 document → 1000 embeddings),
/// so the context holds a `Vec` of values at each stage.
#[derive(Debug, Builder)]
#[builder(
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct ExecutionContext {
    /// Credentials registry for provider authentication.
    credentials: CredentialsRegistry,
    /// Current data items being processed (can expand: 1 input → N outputs).
    #[builder(default)]
    current: Vec<AnyDataValue>,
    /// Named cache slots for data sharing between workflow branches.
    #[builder(default)]
    cache: HashMap<String, Vec<AnyDataValue>>,
    /// Total input items processed in this execution.
    #[builder(default)]
    items_processed: usize,
}

impl ExecutionContextBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.credentials.is_none() {
            return Err("credentials is required".into());
        }
        Ok(())
    }
}

impl ExecutionContext {
    /// Creates a new execution context with the given credentials.
    pub fn new(credentials: CredentialsRegistry) -> Self {
        Self {
            credentials,
            current: Vec::new(),
            cache: HashMap::new(),
            items_processed: 0,
        }
    }

    /// Returns a builder for creating an execution context.
    pub fn builder() -> ExecutionContextBuilder {
        ExecutionContextBuilder::default()
    }

    /// Returns a reference to the credentials registry.
    pub fn credentials(&self) -> &CredentialsRegistry {
        &self.credentials
    }

    /// Sets the current data items being processed.
    pub fn set_current(&mut self, data: Vec<AnyDataValue>) {
        self.current = data;
    }

    /// Sets a single item as current (convenience for input stage).
    pub fn set_current_single(&mut self, data: AnyDataValue) {
        self.current = vec![data];
    }

    /// Takes the current data items, leaving an empty vec in its place.
    pub fn take_current(&mut self) -> Vec<AnyDataValue> {
        std::mem::take(&mut self.current)
    }

    /// Returns a reference to the current data items.
    pub fn current(&self) -> &[AnyDataValue] {
        &self.current
    }

    /// Returns whether there are any current data items.
    pub fn has_current(&self) -> bool {
        !self.current.is_empty()
    }

    /// Returns the number of current data items.
    pub fn current_len(&self) -> usize {
        self.current.len()
    }

    /// Increments the processed items counter.
    pub fn mark_processed(&mut self) {
        self.items_processed += 1;
    }

    /// Returns the number of input items processed.
    pub fn items_processed(&self) -> usize {
        self.items_processed
    }

    /// Clears the current data items.
    pub fn clear(&mut self) {
        self.current.clear();
    }

    /// Writes data to a named cache slot.
    pub fn write_cache(&mut self, name: &str, data: Vec<AnyDataValue>) {
        self.cache.entry(name.to_string()).or_default().extend(data);
    }

    /// Reads data from a named cache slot (returns empty vec if not found).
    pub fn read_cache(&self, name: &str) -> Vec<AnyDataValue> {
        self.cache.get(name).cloned().unwrap_or_default()
    }

    /// Clears a named cache slot.
    pub fn clear_cache(&mut self, name: &str) {
        self.cache.remove(name);
    }

    /// Clears all cache slots.
    pub fn clear_all_caches(&mut self) {
        self.cache.clear();
    }

    /// Returns the names of all cache slots.
    pub fn cache_names(&self) -> Vec<&str> {
        self.cache.keys().map(|s| s.as_str()).collect()
    }
}
