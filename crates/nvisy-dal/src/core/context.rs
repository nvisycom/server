//! Context types for data operations.

/// Context for data operations.
///
/// Provides configuration for read/write operations including target,
/// pagination cursor, and limits.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Target collection, table, bucket, topic, etc.
    pub target: Option<String>,
    /// Cursor for pagination (provider-specific format).
    pub cursor: Option<String>,
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
}
