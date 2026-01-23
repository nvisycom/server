//! Context for relational database operations.

/// Context for relational database operations (Postgres, MySQL).
#[derive(Debug, Clone, Default)]
pub struct RelationalContext {
    /// Target table name.
    pub table: Option<String>,
    /// Last seen cursor value (for keyset pagination).
    pub cursor: Option<String>,
    /// Tiebreaker value for resolving cursor conflicts.
    pub tiebreaker: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

impl RelationalContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the table name.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Sets the cursor value.
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Sets the tiebreaker value.
    pub fn with_tiebreaker(mut self, tiebreaker: impl Into<String>) -> Self {
        self.tiebreaker = Some(tiebreaker.into());
        self
    }

    /// Sets the limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}
