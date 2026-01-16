//! Constants used throughout the application.

/// Number of minutes before expiry to show expiration warnings.
///
/// Used in: `account_api_tokens`
pub const EXPIRY_WARNING_MINUTES: i64 = 15;

/// Number of hours after which a token is considered "long-lived".
///
/// Used in: `account_api_tokens`
pub const LONG_LIVED_THRESHOLD_HOURS: i64 = 24;

/// Number of seconds of grace period for detecting comment edits.
///
/// Used in: `document_comments`
pub const EDIT_GRACE_PERIOD_SECONDS: i64 = 5;

/// Number of hours within which a file is considered "recently uploaded".
///
/// Used in: `document_files`
pub const RECENTLY_UPLOADED_HOURS: i64 = 1;

/// Number of hours within which an invite is considered "recently sent".
///
/// Used in: `workspace_invites`
pub const RECENTLY_SENT_HOURS: i64 = 24;

/// Default notification retention days.
///
/// Used in: `account_notifications`
pub const DEFAULT_RETENTION_DAYS: i32 = 90;

/// Number of dimensions for vector embeddings.
///
/// This value must match the `VECTOR(n)` dimension in the database schema.
/// Currently configured for OpenAI text-embedding-3-small (1536 dimensions).
///
/// Used in: `document_chunks`
pub const EMBEDDING_DIMENSIONS: usize = 1536;
