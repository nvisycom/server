//! Context types for workflow execution.

use derive_builder::Builder;
use nvisy_dal::datatypes::AnyDataValue;

use super::ConnectionRegistry;

/// Execution context for a workflow run.
///
/// Manages the current data items flowing through the pipeline and holds
/// connections for provider access.
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
    /// Connection registry for provider authentication.
    connections: ConnectionRegistry,
    /// Current data items being processed (can expand: 1 input → N outputs).
    #[builder(default)]
    current: Vec<AnyDataValue>,
    /// Total input items processed in this execution.
    #[builder(default)]
    items_processed: usize,
}

impl ExecutionContextBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.connections.is_none() {
            return Err("connections is required".into());
        }
        Ok(())
    }
}

impl ExecutionContext {
    /// Creates a new execution context with the given connections.
    pub fn new(connections: ConnectionRegistry) -> Self {
        Self {
            connections,
            current: Vec::new(),
            items_processed: 0,
        }
    }

    /// Returns a builder for creating an execution context.
    pub fn builder() -> ExecutionContextBuilder {
        ExecutionContextBuilder::default()
    }

    /// Returns a reference to the connection registry.
    pub fn connections(&self) -> &ConnectionRegistry {
        &self.connections
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
}
