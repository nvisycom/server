//! Time-related helper utilities and traits for consistent time handling across models.
//!
//! This module provides time handling capabilities through traits designed for database models.

use jiff::{Span, Timestamp, Unit};

/// Common time duration constants used throughout the application.
mod constants {
    use jiff::Span;

    /// Duration for considering something "recently created" (24 hours).
    pub fn recently_created() -> Span {
        Span::new().hours(24)
    }

    /// Duration for considering something "recently updated" (1 hour).
    pub fn recently_updated() -> Span {
        Span::new().hours(1)
    }

    /// Duration for considering an account "recently active" (30 days).
    pub fn recently_active() -> Span {
        Span::new().days(30)
    }
}

/// Returns whether a timestamp is within the specified duration from now.
pub fn is_within_duration(timestamp: Timestamp, duration: Span) -> bool {
    let now = Timestamp::now();
    now.since(timestamp)
        .map(|s| s.total(Unit::Second).ok() <= duration.total(Unit::Second).ok())
        .unwrap_or(false)
}

/// Trait for models that have creation timestamps.
pub trait HasCreatedAt {
    /// Returns the creation timestamp.
    fn created_at(&self) -> Timestamp;

    /// Returns whether the entity was created recently.
    fn is_recently_created(&self) -> bool {
        is_within_duration(self.created_at(), constants::recently_created())
    }

    /// Returns whether the entity was created within the specified duration.
    fn was_created_within(&self, duration: Span) -> bool {
        is_within_duration(self.created_at(), duration)
    }

    /// Returns the age of the entity since creation.
    fn creation_age(&self) -> Span {
        Timestamp::now()
            .since(self.created_at())
            .unwrap_or_else(|_| Span::new())
    }
}

/// Trait for models that have update timestamps.
pub trait HasUpdatedAt {
    /// Returns the last update timestamp.
    fn updated_at(&self) -> Timestamp;

    /// Returns whether the entity was updated recently.
    fn is_recently_updated(&self) -> bool {
        is_within_duration(self.updated_at(), constants::recently_updated())
    }

    /// Returns whether the entity was updated within the specified duration.
    fn was_updated_within(&self, duration: Span) -> bool {
        is_within_duration(self.updated_at(), duration)
    }

    /// Returns the time since the last update.
    fn time_since_update(&self) -> Span {
        Timestamp::now()
            .since(self.updated_at())
            .unwrap_or_else(|_| Span::new())
    }
}

/// Trait for models that support soft deletion.
pub trait HasDeletedAt {
    /// Returns the deletion timestamp if the entity is soft-deleted.
    fn deleted_at(&self) -> Option<Timestamp>;

    /// Returns whether the entity is soft-deleted.
    fn is_deleted(&self) -> bool {
        self.deleted_at().is_some()
    }

    /// Returns whether the entity is active (not deleted).
    fn is_active(&self) -> bool {
        !self.is_deleted()
    }

    /// Returns the time since deletion, if deleted.
    fn time_since_deletion(&self) -> Option<Span> {
        self.deleted_at().map(|deleted_at| {
            Timestamp::now()
                .since(deleted_at)
                .unwrap_or_else(|_| Span::new())
        })
    }
}

/// Trait for models that have expiration timestamps.
pub trait HasExpiresAt {
    /// Returns the expiration timestamp, or None if the entity never expires.
    fn expires_at(&self) -> Option<Timestamp>;

    /// Returns whether the entity has expired.
    /// Returns false if the entity never expires.
    fn is_expired(&self) -> bool {
        match self.expires_at() {
            Some(expires_at) => Timestamp::now() > expires_at,
            None => false,
        }
    }

    /// Returns whether the entity is still valid (not expired).
    fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Returns the time remaining until expiration.
    /// Returns None if the entity never expires or has already expired.
    fn time_until_expiry(&self) -> Option<Span> {
        let expires_at = self.expires_at()?;
        let now = Timestamp::now();
        if expires_at > now {
            expires_at.since(now).ok()
        } else {
            None
        }
    }

    /// Returns whether the entity is expiring soon (within specified duration).
    /// Returns false if the entity never expires.
    fn is_expiring_soon(&self, threshold: Span) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.total(Unit::Second).ok() <= threshold.total(Unit::Second).ok()
        } else {
            false
        }
    }
}

/// Trait for models that track last activity timestamps.
pub trait HasLastActivityAt {
    /// Returns the last activity timestamp.
    fn last_activity_at(&self) -> Option<Timestamp>;

    /// Returns whether there was recent activity.
    fn has_recent_activity(&self) -> bool {
        self.has_activity_within(constants::recently_active())
    }

    /// Returns whether there was activity within the specified duration.
    fn has_activity_within(&self, duration: Span) -> bool {
        if let Some(last_activity) = self.last_activity_at() {
            is_within_duration(last_activity, duration)
        } else {
            false
        }
    }

    /// Returns the duration since last activity.
    fn time_since_last_activity(&self) -> Option<Span> {
        self.last_activity_at().map(|last_activity| {
            Timestamp::now()
                .since(last_activity)
                .unwrap_or_else(|_| Span::new())
        })
    }
}
