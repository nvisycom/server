//! Account-related constraint violation error handlers.

use nvisy_postgres::types::{
    AccountApiTokenConstraints, AccountConstraints, AccountNotificationConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<AccountConstraints> for Error<'static> {
    fn from(c: AccountConstraints) -> Self {
        let error = match c {
            AccountConstraints::UsernameLength => ErrorKind::BadRequest
                .with_message("Handle must be between 3 and 32 characters long"),
            AccountConstraints::UsernameFormat => ErrorKind::BadRequest
                .with_message("Handle must be lowercase alphanumeric with single internal dashes"),
            AccountConstraints::EmailUnique => {
                ErrorKind::Conflict.with_message("An account with this email already exists")
            }
            AccountConstraints::UsernameUnique => {
                ErrorKind::Conflict.with_message("Handle is already taken")
            }
            AccountConstraints::DisplayNameLength => ErrorKind::BadRequest
                .with_message("Display name must be between 2 and 32 characters long"),
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
            AccountConstraints::TimezoneFormat => {
                ErrorKind::BadRequest.with_message("Invalid timezone format")
            }
            AccountConstraints::LocaleFormat => {
                ErrorKind::BadRequest.with_message("Invalid locale format")
            }
            AccountConstraints::UpdatedAfterCreated
            | AccountConstraints::DeletedAfterCreated
            | AccountConstraints::DeletedAfterUpdated
            | AccountConstraints::PasswordChangedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountConstraints::SuspendedNotAdmin => {
                ErrorKind::BadRequest.with_message("Admin accounts cannot be suspended")
            }
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
            AccountApiTokenConstraints::ExpiredAfterIssued
            | AccountApiTokenConstraints::DeletedAfterIssued
            | AccountApiTokenConstraints::LastUsedAfterIssued => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("account_api_token")
    }
}

impl From<AccountNotificationConstraints> for Error<'static> {
    fn from(constraint: AccountNotificationConstraints) -> Self {
        let error = match constraint {
            AccountNotificationConstraints::ParamsSize => ErrorKind::BadRequest
                .with_message("Notification params must be between 2 and 4096 bytes"),
            // Server-controlled timestamps; a violation is a server invariant break.
            AccountNotificationConstraints::ExpiresAfterCreated
            | AccountNotificationConstraints::ReadAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
        };

        error.with_resource("notification")
    }
}
