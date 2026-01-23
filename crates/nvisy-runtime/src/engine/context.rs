//! Context types for workflow execution.

use derive_builder::Builder;
use nvisy_dal::AnyDataValue;

use crate::provider::CredentialsRegistry;

/// Context for provider operations during compilation and execution.
///
/// Provides configuration for read/write operations including target,
/// pagination cursor, and limits.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Target collection, table, bucket, topic, etc.
    pub target: Option<String>,
    /// Cursor for pagination (provider-specific format).
    pub cursor: Option<String>,
    /// Tiebreaker for pagination conflicts.
    pub tiebreaker: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

impl Context {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the target.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Sets the cursor for pagination.
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Sets the tiebreaker for pagination.
    pub fn with_tiebreaker(mut self, tiebreaker: impl Into<String>) -> Self {
        self.tiebreaker = Some(tiebreaker.into());
        self
    }

    /// Sets the limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Returns the target, if set.
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Returns the cursor, if set.
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Returns the limit, if set.
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Converts to an ObjectContext for object storage providers.
    pub fn to_object_context(&self) -> nvisy_dal::ObjectContext {
        let mut ctx = nvisy_dal::ObjectContext::new();
        if let Some(ref prefix) = self.target {
            ctx = ctx.with_prefix(prefix.clone());
        }
        if let Some(ref token) = self.cursor {
            ctx = ctx.with_token(token.clone());
        }
        if let Some(limit) = self.limit {
            ctx = ctx.with_limit(limit);
        }
        ctx
    }

    /// Converts to a RelationalContext for relational database providers.
    pub fn to_relational_context(&self) -> nvisy_dal::RelationalContext {
        let mut ctx = nvisy_dal::RelationalContext::new();
        if let Some(ref table) = self.target {
            ctx = ctx.with_table(table.clone());
        }
        if let Some(ref cursor) = self.cursor {
            ctx = ctx.with_cursor(cursor.clone());
        }
        if let Some(ref tiebreaker) = self.tiebreaker {
            ctx = ctx.with_tiebreaker(tiebreaker.clone());
        }
        if let Some(limit) = self.limit {
            ctx = ctx.with_limit(limit);
        }
        ctx
    }
}

/// Execution context for a workflow run.
///
/// Manages the current data items flowing through the pipeline and holds
/// credentials for provider access.
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
}
