//! Context for object storage operations.

/// Context for object storage operations (S3, GCS, Azure Blob).
#[derive(Debug, Clone, Default)]
pub struct ObjectContext {
    /// Path prefix for listing objects.
    pub prefix: Option<String>,
    /// Continuation token for pagination.
    pub token: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

impl ObjectContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Sets the continuation token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Sets the limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}
