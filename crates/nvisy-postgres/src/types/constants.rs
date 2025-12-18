//! Constants used throughout the application.

/// Database-related constants.
pub mod database {
    /// Default pagination limit.
    pub const DEFAULT_PAGE_SIZE: i64 = 50;

    /// Maximum pagination limit.
    pub const MAX_PAGE_SIZE: i64 = 1000;
}

/// Security-related constants.
pub mod security {
    /// Default bcrypt cost for password hashing.
    pub const DEFAULT_BCRYPT_COST: u32 = 12;

    /// Maximum number of active sessions per user.
    pub const MAX_SESSIONS_PER_USER: i32 = 10;
}

/// File and storage related constants.
pub mod storage {
    /// Maximum file size in MB.
    pub const MAX_FILE_SIZE_MB: i32 = 100;

    /// Maximum total storage per project in MB.
    pub const MAX_PROJECT_STORAGE_MB: i32 = 1000;
}

/// Notification and communication constants.
pub mod notification {
    /// Default notification retention days.
    pub const DEFAULT_RETENTION_DAYS: i32 = 90;

    /// Number of days within which a notification is considered "recent".
    pub const RECENT_DAYS: i64 = 7;
}

/// Constants related to account security and behavior.
pub mod account {
    /// Maximum number of consecutive failed login attempts before account lockout.
    pub const MAX_FAILED_LOGIN_ATTEMPTS: i32 = 5;

    /// Number of days after which a password change reminder should be shown.
    pub const PASSWORD_CHANGE_REMINDER_DAYS: i64 = 90;

    /// Number of days within which an account is considered "recently active".
    pub const RECENT_ACTIVITY_DAYS: i64 = 30;

    /// Number of hours within which an account is considered "recently created".
    pub const RECENTLY_CREATED_HOURS: i64 = 24;
}

/// Constants related to API tokens and sessions.
pub mod token {
    /// Number of minutes within which a token is considered "recently used".
    pub const RECENT_USE_MINUTES: i64 = 30;

    /// Number of minutes before expiry to show expiration warnings.
    pub const EXPIRY_WARNING_MINUTES: i64 = 15;

    /// Number of hours after which a token is considered "long-lived".
    pub const LONG_LIVED_THRESHOLD_HOURS: i64 = 24;

    /// Number of hours within which a token is considered "recently created".
    pub const RECENTLY_CREATED_HOURS: i64 = 1;
}

/// Constants related to comments and discussions.
pub mod comment {
    /// Number of seconds of grace period for detecting comment edits.
    pub const EDIT_GRACE_PERIOD_SECONDS: i64 = 5;

    /// Number of hours within which a comment is considered "recently created".
    pub const RECENTLY_CREATED_HOURS: i64 = 24;

    /// Number of hours within which a comment is considered "recently updated".
    pub const RECENTLY_UPDATED_HOURS: i64 = 1;
}

/// Constants related to projects and project management.
pub mod project {
    /// Number of days within which project access is considered "recent".
    pub const RECENT_ACCESS_DAYS: i64 = 7;

    /// Number of hours within which a project is considered "recently created".
    pub const RECENTLY_CREATED_HOURS: i64 = 24;
}

/// Constants related to documents and document processing.
pub mod document {
    /// Number of hours within which a document is considered "recently created".
    pub const RECENTLY_CREATED_HOURS: i64 = 24;

    /// Number of hours within which a document is considered "recently updated".
    pub const RECENTLY_UPDATED_HOURS: i64 = 1;
}

/// Constants related to file processing and storage.
pub mod file {
    /// Number of hours within which a file is considered "recently uploaded".
    pub const RECENTLY_UPLOADED_HOURS: i64 = 1;

    /// Number of days within which processing status is considered "stale".
    pub const STALE_PROCESSING_DAYS: i64 = 1;
}

/// Constants related to invitations and membership.
pub mod invite {
    /// Number of days an invitation remains valid by default.
    pub const DEFAULT_EXPIRY_DAYS: i64 = 7;

    /// Number of hours within which an invite is considered "recently sent".
    pub const RECENTLY_SENT_HOURS: i64 = 24;
}
