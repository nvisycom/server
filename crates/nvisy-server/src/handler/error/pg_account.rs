//! Account-related constraint violation error handlers.

use nvisy_postgres::types::{
    AccountActionTokenConstraints, AccountApiTokenConstraints, AccountConstraints,
    AccountNotificationConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<AccountConstraints> for Error<'static> {
    fn from(c: AccountConstraints) -> Self {
        let error = match c {
            AccountConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Display name must be between 2 and 100 characters long"),
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
            AccountConstraints::TimezoneFormat => {
                ErrorKind::BadRequest.with_message("Invalid timezone format")
            }
            AccountConstraints::LocaleFormat => {
                ErrorKind::BadRequest.with_message("Invalid locale format")
            }
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

impl From<AccountApiTokenConstraints> for Error<'static> {
    fn from(c: AccountApiTokenConstraints) -> Self {
        let error = match c {
            AccountApiTokenConstraints::NameNotEmpty => {
                ErrorKind::BadRequest.with_message("Token name cannot be empty")
            }
            AccountApiTokenConstraints::NameLength => {
                ErrorKind::BadRequest.with_message("Token name is too long")
            }
            AccountApiTokenConstraints::DescriptionLength => {
                ErrorKind::BadRequest.with_message("Token description is too long")
            }
            AccountApiTokenConstraints::RegionCodeValid => {
                ErrorKind::BadRequest.with_message("Invalid region code")
            }
            AccountApiTokenConstraints::CountryCodeValid => {
                ErrorKind::BadRequest.with_message("Invalid country code")
            }
            AccountApiTokenConstraints::UserAgentNotEmpty => {
                ErrorKind::BadRequest.with_message("User agent cannot be empty")
            }
            AccountApiTokenConstraints::ExpiredAfterIssued
            | AccountApiTokenConstraints::DeletedAfterIssued
            | AccountApiTokenConstraints::LastUsedAfterIssued => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountApiTokenConstraints::AccessSeqUnique => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountApiTokenConstraints::RefreshSeqUnique => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("account_api_token")
    }
}

impl From<AccountActionTokenConstraints> for Error<'static> {
    fn from(c: AccountActionTokenConstraints) -> Self {
        let error = match c {
            AccountActionTokenConstraints::PrimaryKey => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountActionTokenConstraints::ActionDataSize => {
                ErrorKind::BadRequest.with_message("Action data size is invalid")
            }
            AccountActionTokenConstraints::ExpiredAfterIssued
            | AccountActionTokenConstraints::UsedAfterIssued
            | AccountActionTokenConstraints::UsedBeforeExpired => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("account_action_token")
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
