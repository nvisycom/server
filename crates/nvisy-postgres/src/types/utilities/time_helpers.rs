//! Time-related helper utilities and traits for consistent time handling across models.
//!
//! This module provides time handling capabilities through traits designed for database models.

use time::{Duration, OffsetDateTime};

/// Common time duration constants used throughout the application.
pub mod constants {
    use time::Duration;

    /// Duration for considering something "recently created" (24 hours).
    pub const RECENTLY_CREATED: Duration = Duration::hours(24);

    /// Duration for considering something "recently updated" (1 hour).
    pub const RECENTLY_UPDATED: Duration = Duration::hours(1);

    /// Duration for considering an account "recently active" (30 days).
    pub const RECENTLY_ACTIVE: Duration = Duration::days(30);
}

/// Returns whether a timestamp is within the specified duration from now.
pub fn is_within_duration(timestamp: OffsetDateTime, duration: Duration) -> bool {
    let now = OffsetDateTime::now_utc();
    (now - timestamp) <= duration
}

/// Trait for models that have creation timestamps.
pub trait HasCreatedAt {
    /// Returns the creation timestamp.
    fn created_at(&self) -> OffsetDateTime;

    /// Returns whether the entity was created recently.
    fn is_recently_created(&self) -> bool {
        is_within_duration(self.created_at(), constants::RECENTLY_CREATED)
    }

    /// Returns whether the entity was created within the specified duration.
    fn was_created_within(&self, duration: Duration) -> bool {
        is_within_duration(self.created_at(), duration)
    }

    /// Returns the age of the entity since creation.
    fn creation_age(&self) -> Duration {
        OffsetDateTime::now_utc() - self.created_at()
    }
}

/// Trait for models that have update timestamps.
pub trait HasUpdatedAt {
    /// Returns the last update timestamp.
    fn updated_at(&self) -> OffsetDateTime;

    /// Returns whether the entity was updated recently.
    fn is_recently_updated(&self) -> bool {
        is_within_duration(self.updated_at(), constants::RECENTLY_UPDATED)
    }

    /// Returns whether the entity was updated within the specified duration.
    fn was_updated_within(&self, duration: Duration) -> bool {
        is_within_duration(self.updated_at(), duration)
    }

    /// Returns the time since the last update.
    fn time_since_update(&self) -> Duration {
        OffsetDateTime::now_utc() - self.updated_at()
    }
}

/// Trait for models that support soft deletion.
pub trait HasDeletedAt {
    /// Returns the deletion timestamp if the entity is soft-deleted.
    fn deleted_at(&self) -> Option<OffsetDateTime>;

    /// Returns whether the entity is soft-deleted.
    fn is_deleted(&self) -> bool {
        self.deleted_at().is_some()
    }

    /// Returns whether the entity is active (not deleted).
    fn is_active(&self) -> bool {
        !self.is_deleted()
    }

    /// Returns the time since deletion, if deleted.
    fn time_since_deletion(&self) -> Option<Duration> {
        self.deleted_at()
            .map(|deleted_at| OffsetDateTime::now_utc() - deleted_at)
    }
}

/// Trait for models that have expiration timestamps.
pub trait HasExpiresAt {
    /// Returns the expiration timestamp.
    fn expires_at(&self) -> OffsetDateTime;

    /// Returns whether the entity has expired.
    fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() > self.expires_at()
    }

    /// Returns whether the entity is still valid (not expired).
    fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Returns the time remaining until expiration.
    fn time_until_expiry(&self) -> Option<Duration> {
        let now = OffsetDateTime::now_utc();
        if self.expires_at() > now {
            Some(self.expires_at() - now)
        } else {
            None
        }
    }

    /// Returns whether the entity is expiring soon (within specified duration).
    fn is_expiring_soon(&self, threshold: Duration) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining <= threshold
        } else {
            false
        }
    }
}

/// Trait for models that track last activity timestamps.
pub trait HasLastActivityAt {
    /// Returns the last activity timestamp.
    fn last_activity_at(&self) -> Option<OffsetDateTime>;

    /// Returns whether there was recent activity.
    fn has_recent_activity(&self) -> bool {
        self.has_activity_within(constants::RECENTLY_ACTIVE)
    }

    /// Returns whether there was activity within the specified duration.
    fn has_activity_within(&self, duration: Duration) -> bool {
        if let Some(last_activity) = self.last_activity_at() {
            is_within_duration(last_activity, duration)
        } else {
            false
        }
    }

    /// Returns the duration since last activity.
    fn time_since_last_activity(&self) -> Option<Duration> {
        self.last_activity_at()
            .map(|last_activity| OffsetDateTime::now_utc() - last_activity)
    }
}
