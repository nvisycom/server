//! Database migration management.
//!
//! This module provides comprehensive database migration functionality through
//! an extension trait pattern. It includes migration execution, rollback capabilities,
//! and status monitoring with detailed error handling and observability.
//!
//! ## Features
//!
//! - **Migration Execution**: Apply pending migrations automatically
//! - **Rollback Support**: Undo migrations when needed
//! - **Status Monitoring**: Track migration progress and state
//! - **Schema Integrity**: Verify database schema consistency
//! - **Observability**: Comprehensive logging and tracing

mod client_ext;
pub(crate) mod custom_hooks;
mod migrate_result;
mod run_migration;
mod run_utility;

// Re-export main types for convenience
pub use client_ext::PgClientMigrationExt;
pub use migrate_result::{MigrationResult, MigrationStatus};
pub use run_migration::run_pending_migrations;
pub use run_utility::{get_applied_migrations, get_migration_status, verify_schema_integrity};
