//! Constants used throughout the application.

/// Number of hours within which an invite is considered "recently sent".
///
/// Used in: `workspace_invites`
pub const RECENTLY_SENT_HOURS: i64 = 24;

/// Default notification retention days.
///
/// Used in: `account_notifications`
pub const DEFAULT_RETENTION_DAYS: i32 = 90;
