//! Account notification constraint violations.

use std::fmt;

/// Constraint violations for the account_notifications table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountNotificationConstraints {
    /// Title length must be between 1 and 200 characters
    TitleLength,
    /// Message length must be between 1 and 1000 characters
    MessageLength,
    /// Related type length must be between 1 and 50 characters
    RelatedTypeLength,
    /// Metadata JSON size must be between 2 and 4096 bytes
    MetadataSize,
    /// Expiration timestamp must be after creation timestamp
    ExpiresAfterCreated,
    /// Read timestamp must be after creation timestamp
    ReadAfterCreated,
}

impl AccountNotificationConstraints {
    /// Returns the PostgreSQL constraint name.
    #[must_use]
    pub const fn constraint_name(self) -> &'static str {
        match self {
            Self::TitleLength => "account_notifications_title_length",
            Self::MessageLength => "account_notifications_message_length",
            Self::RelatedTypeLength => "account_notifications_related_type_length",
            Self::MetadataSize => "account_notifications_metadata_size",
            Self::ExpiresAfterCreated => "account_notifications_expires_after_created",
            Self::ReadAfterCreated => "account_notifications_read_after_created",
        }
    }

    /// Returns a user-friendly error message for this constraint violation.
    #[must_use]
    pub const fn error_message(self) -> &'static str {
        match self {
            Self::TitleLength => "Notification title must be between 1 and 200 characters",
            Self::MessageLength => "Notification message must be between 1 and 1000 characters",
            Self::RelatedTypeLength => "Related type must be between 1 and 50 characters",
            Self::MetadataSize => "Notification metadata must be between 2 and 4096 bytes",
            Self::ExpiresAfterCreated => "Expiration time must be after creation time",
            Self::ReadAfterCreated => "Read time must be after creation time",
        }
    }

    /// Attempts to parse a constraint name into an `AccountNotificationConstraints` variant.
    #[must_use]
    pub fn from_constraint_name(name: &str) -> Option<Self> {
        match name {
            "account_notifications_title_length" => Some(Self::TitleLength),
            "account_notifications_message_length" => Some(Self::MessageLength),
            "account_notifications_related_type_length" => Some(Self::RelatedTypeLength),
            "account_notifications_metadata_size" => Some(Self::MetadataSize),
            "account_notifications_expires_after_created" => Some(Self::ExpiresAfterCreated),
            "account_notifications_read_after_created" => Some(Self::ReadAfterCreated),
            _ => None,
        }
    }
}

impl AccountNotificationConstraints {
    /// Categorizes this constraint by its purpose.
    #[must_use]
    pub const fn categorize(self) -> super::ConstraintCategory {
        use super::ConstraintCategory;

        match self {
            Self::TitleLength
            | Self::MessageLength
            | Self::RelatedTypeLength
            | Self::MetadataSize => ConstraintCategory::Validation,
            Self::ExpiresAfterCreated | Self::ReadAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl fmt::Display for AccountNotificationConstraints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error_message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_name_roundtrip() {
        let constraints = [
            AccountNotificationConstraints::TitleLength,
            AccountNotificationConstraints::MessageLength,
            AccountNotificationConstraints::RelatedTypeLength,
            AccountNotificationConstraints::MetadataSize,
            AccountNotificationConstraints::ExpiresAfterCreated,
            AccountNotificationConstraints::ReadAfterCreated,
        ];

        for constraint in constraints {
            let name = constraint.constraint_name();
            let parsed = AccountNotificationConstraints::from_constraint_name(name);
            assert_eq!(Some(constraint), parsed);
        }
    }

    #[test]
    fn test_unknown_constraint() {
        assert_eq!(
            None,
            AccountNotificationConstraints::from_constraint_name("unknown_constraint")
        );
    }
}
