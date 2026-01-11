//! Avatar bucket constants for NATS object storage.

use std::time::Duration;

/// Bucket name for account avatars.
pub const AVATAR_BUCKET: &str = "ACCOUNT_AVATARS";

/// Maximum age for avatars (none - retained indefinitely).
pub const AVATAR_MAX_AGE: Option<Duration> = None;
