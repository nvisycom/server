//! Account-related constraint violation error handlers.

use nvisy_postgres::types::{
    AccountConstraints, AccountNotificationConstraints, AccountSessionConstraints,
    AccountTokenConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<AccountConstraints> for Error<'static> {
    fn from(c: AccountConstraints) -> Self {
        let error = match c {
            AccountConstraints::DisplayNameLengthMin => ErrorKind::BadRequest
                .with_message("Display name must be at least 2 characters long"),
            AccountConstraints::DisplayNameLengthMax => {
                ErrorKind::BadRequest.with_message("Display name cannot exceed 32 characters")
            }
            AccountConstraints::DisplayNameNotEmpty => {
                ErrorKind::BadRequest.with_message("Display name cannot be empty")
            }
            AccountConstraints::EmailFormat => {
                ErrorKind::BadRequest.with_message("Invalid email format")
            }
            AccountConstraints::EmailLengthMax => {
                ErrorKind::BadRequest.with_message("Email address is too long")
            }
            AccountConstraints::PasswordHashNotEmpty => {
                ErrorKind::BadRequest.with_message("Password cannot be empty")
            }
            AccountConstraints::PasswordHashLengthMin => {
                ErrorKind::BadRequest.with_message("Password hash is too short")
            }
            AccountConstraints::CompanyNameLengthMax => {
                ErrorKind::BadRequest.with_message("Company name is too long")
            }
            AccountConstraints::PhoneNumberLengthMax => {
                ErrorKind::BadRequest.with_message("Phone number is too long")
            }
            AccountConstraints::TimezoneFormat => {
                ErrorKind::BadRequest.with_message("Invalid timezone format")
            }
            AccountConstraints::LocaleFormat => {
                ErrorKind::BadRequest.with_message("Invalid locale format")
            }
            AccountConstraints::FailedLoginAttemptsMin => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountConstraints::FailedLoginAttemptsMax => {
                ErrorKind::BadRequest.with_message("Too many failed login attempts")
            }
            AccountConstraints::LockedUntilFuture => ErrorKind::InternalServerError.into_error(),
            AccountConstraints::UpdatedAfterCreated
            | AccountConstraints::DeletedAfterCreated
            | AccountConstraints::DeletedAfterUpdated
            | AccountConstraints::PasswordChangedAfterCreated
            | AccountConstraints::LastLoginAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountConstraints::SuspendedNotAdmin => {
                ErrorKind::BadRequest.with_message("Admin accounts cannot be suspended")
            }
            AccountConstraints::EmailAddressUnique => ErrorKind::Conflict
                .with_message("An account with this email address already exists"),
        };

        error.with_resource("account")
    }
}

impl From<AccountSessionConstraints> for Error<'static> {
    fn from(c: AccountSessionConstraints) -> Self {
        let error = match c {
            AccountSessionConstraints::RegionCodeValid => {
                ErrorKind::BadRequest.with_message("Invalid region code")
            }
            AccountSessionConstraints::CountryCodeValid => {
                ErrorKind::BadRequest.with_message("Invalid country code")
            }
            AccountSessionConstraints::UserAgentNotEmpty => {
                ErrorKind::BadRequest.with_message("User agent cannot be empty")
            }
            AccountSessionConstraints::ExpiredAfterIssued
            | AccountSessionConstraints::DeletedAfterIssued
            | AccountSessionConstraints::LastUsedAfterIssued => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountSessionConstraints::AccessSeqUnique => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountSessionConstraints::RefreshSeqUnique => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("account_session")
    }
}

impl From<AccountTokenConstraints> for Error<'static> {
    fn from(c: AccountTokenConstraints) -> Self {
        let error = match c {
            AccountTokenConstraints::PrimaryKey => ErrorKind::InternalServerError.into_error(),
            AccountTokenConstraints::ActionDataSizeMin => {
                ErrorKind::BadRequest.with_message("Action data is too small")
            }
            AccountTokenConstraints::ActionDataSizeMax => {
                ErrorKind::BadRequest.with_message("Action data exceeds maximum allowed size")
            }
            AccountTokenConstraints::UserAgentNotEmpty => {
                ErrorKind::BadRequest.with_message("User agent cannot be empty")
            }
            AccountTokenConstraints::AttemptCountMin => ErrorKind::InternalServerError.into_error(),
            AccountTokenConstraints::AttemptCountMax => {
                ErrorKind::BadRequest.with_message("Maximum attempts exceeded")
            }
            AccountTokenConstraints::MaxAttemptsMin | AccountTokenConstraints::MaxAttemptsMax => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountTokenConstraints::ExpiredAfterIssued
            | AccountTokenConstraints::UsedAfterIssued
            | AccountTokenConstraints::UsedBeforeExpired => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("account_token")
    }
}

impl From<AccountNotificationConstraints> for Error<'static> {
    fn from(constraint: AccountNotificationConstraints) -> Self {
        let error = match constraint {
            AccountNotificationConstraints::TitleLength => ErrorKind::BadRequest
                .with_message("Notification title must be between 1 and 200 characters"),
            AccountNotificationConstraints::MessageLength => ErrorKind::BadRequest
                .with_message("Notification message must be between 1 and 1000 characters"),
            AccountNotificationConstraints::RelatedTypeLength => ErrorKind::BadRequest
                .with_message("Related type must be between 1 and 50 characters"),
            AccountNotificationConstraints::MetadataSize => ErrorKind::BadRequest
                .with_message("Notification metadata must be between 2 and 4096 bytes"),
            AccountNotificationConstraints::ExpiresAfterCreated => ErrorKind::BadRequest
                .with_message("Notification expiration time must be after creation time"),
            AccountNotificationConstraints::ReadAfterCreated => ErrorKind::BadRequest
                .with_message("Notification read time must be after creation time"),
        };

        error.with_resource("notification")
    }
}
