//! Type definitions for database migration operations.
//!
//! This module contains data structures used to represent the state and
//! results of database migration operations, providing detailed information
//! for monitoring and debugging migration processes.

use std::time::Duration;

/// Migration status information.
///
/// This struct provides comprehensive information about the current state
/// of database migrations, including which migrations have been applied
/// and which are still pending.
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_postgres::client::migrate::MigrationStatus;
///
/// fn print_status(status: &MigrationStatus) {
///     println!("Database migrations: {}/{} applied",
///              status.applied_count, status.total_migrations);
///
///     if status.is_up_to_date {
///         println!("✓ Database is up to date");
///     } else {
///         println!("⚠ {} migrations pending", status.pending_count);
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatus {
    /// List of applied migration versions in chronological order
    pub applied_versions: Vec<String>,
    /// List of pending migration versions
    pub pending_versions: Vec<String>,
}

impl MigrationStatus {
    /// Creates a new migration status.
    pub fn new(
        applied_versions: impl Into<Vec<String>>,
        pending_versions: impl Into<Vec<String>>,
    ) -> Self {
        Self {
            applied_versions: applied_versions.into(),
            pending_versions: pending_versions.into(),
        }
    }

    /// Returns the progress ratio (0.0 to 1.0) of applied migrations.
    pub fn progress_ratio(&self) -> f64 {
        let total_migrations = self.total_migrations();
        if total_migrations == 0 {
            1.0
        } else {
            self.applied_migrations() as f64 / total_migrations as f64
        }
    }

    /// Returns the last applied migration version, if any.
    pub fn last_applied_version(&self) -> Option<&str> {
        self.applied_versions.last().map(|s| s.as_str())
    }

    /// Returns the next pending migration version, if any.
    pub fn next_pending_version(&self) -> Option<&str> {
        self.pending_versions.first().map(|s| s.as_str())
    }

    /// Returns the number of applied migrations.
    #[inline]
    pub fn applied_migrations(&self) -> usize {
        self.applied_versions.len()
    }

    /// Returns the number of pending migrations.
    #[inline]
    pub fn pending_migrations(&self) -> usize {
        self.pending_versions.len()
    }

    /// Returns the total number of migrations.
    #[inline]
    pub fn total_migrations(&self) -> usize {
        self.applied_migrations() + self.pending_migrations()
    }

    /// Returns true if all migrations have been applied.
    #[inline]
    pub fn is_up_to_date(&self) -> bool {
        self.pending_versions.is_empty()
    }
}

/// Migration operation result information.
///
/// This struct contains detailed information about the outcome of a migration
/// operation, including performance metrics and error details.
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_postgres::client::migrate::MigrationResult;
/// use std::time::Duration;
///
/// fn handle_migration_result(result: MigrationResult) {
///     if result.success {
///         println!("✓ Applied {} migrations in {:?}",
///                  result.migrations_processed, result.duration);
///     } else {
///         eprintln!("✗ Migration failed: {}",
///                   result.error_message.unwrap_or_default());
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationResult {
    /// Total duration of the migration operation
    pub duration: Duration,
    /// List of migration versions that were processed
    pub processed_versions: Vec<String>,
    /// Error message if the operation failed
    pub error_message: Option<String>,
}

impl MigrationResult {
    /// Creates a successful migration result.
    pub fn success(duration: Duration, processed_versions: Vec<String>) -> Self {
        Self {
            duration,
            processed_versions,
            error_message: None,
        }
    }

    /// Creates a failed migration result.
    pub fn failure(duration: Duration, error_message: String) -> Self {
        Self {
            duration,
            processed_versions: vec![],
            error_message: Some(error_message),
        }
    }

    /// Returns the average time per migration processed.
    pub fn average_time_per_migration(&self) -> Option<Duration> {
        let processed = self.processed_versions.len() as u32;
        if processed > 0 {
            Some(self.duration / processed)
        } else {
            None
        }
    }

    /// Returns whether this result indicates a successful operation with no migrations processed.
    pub fn is_no_op(&self) -> bool {
        self.error_message.is_none() && self.processed_versions.is_empty()
    }

    /// Returns the last processed migration version, if any.
    pub fn last_processed_version(&self) -> Option<&str> {
        self.processed_versions.last().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_migration_status_calculations() {
        let applied = vec!["001".to_string(), "002".to_string()];
        let pending = vec!["003".to_string(), "004".to_string()];
        let status = MigrationStatus::new(applied.clone(), pending.clone());

        assert_eq!(status.progress_ratio(), 0.5);
        assert_eq!(status.last_applied_version(), Some("002"));
        assert_eq!(status.next_pending_version(), Some("003"));
    }

    #[test]
    fn test_migration_status_up_to_date() {
        let applied = vec!["001".to_string(), "002".to_string()];
        let pending = vec![];

        let status = MigrationStatus::new(applied, pending);
        assert_eq!(status.progress_ratio(), 1.0);
        assert_eq!(status.next_pending_version(), None);
        assert!(status.is_up_to_date());
    }

    #[test]
    fn test_migration_result_no_op() {
        let result = MigrationResult::success(Duration::from_millis(100), vec![]);

        assert!(result.is_no_op());
        assert_eq!(result.average_time_per_migration(), None);
        assert_eq!(result.last_processed_version(), None);
    }
}
