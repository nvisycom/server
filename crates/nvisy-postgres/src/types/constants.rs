//! Constants used throughout the application.

/// Number of minutes before expiry to show expiration warnings.
///
/// Used in: `account_api_tokens`
pub const EXPIRY_WARNING_MINUTES: i64 = 15;

/// Number of hours after which a token is considered "long-lived".
///
/// Used in: `account_api_tokens`
pub const LONG_LIVED_THRESHOLD_HOURS: i64 = 24;

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
