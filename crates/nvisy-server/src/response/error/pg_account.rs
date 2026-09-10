//! Account-related constraint violation error handlers.

use nvisy_postgres::types::{
    AccountApiTokenConstraints, AccountConstraints, AccountIdentityConstraints,
    AccountNotificationConstraints,
};

use super::{Error, ErrorKind};

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
            AccountConstraints::TimezoneFormat => {
                ErrorKind::BadRequest.with_message("Invalid timezone format")
            }
            AccountConstraints::LocaleFormat => {
                ErrorKind::BadRequest.with_message("Invalid locale format")
            }
            AccountConstraints::SuspendedNotAdmin => {
                ErrorKind::BadRequest.with_message("Admin accounts cannot be suspended")
            }
        };

        error.with_resource("account")
    }
}

impl From<AccountIdentityConstraints> for Error<'static> {
    fn from(c: AccountIdentityConstraints) -> Self {
        let error = match c {
            // The shape checks and uniqueness on identities guard invariants the
            // handlers already enforce, so a violation is a server-side bug, not a
            // client input error, except the subject collision below.
            AccountIdentityConstraints::PasswordShape
            | AccountIdentityConstraints::OidcShape
            | AccountIdentityConstraints::AccountProviderUnique => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountIdentityConstraints::ProviderSubjectUnique => ErrorKind::Conflict
                .with_message("This identity is already linked to another account"),
        };

        error.with_resource("account_identity")
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
        };

        error.with_resource("account_api_token")
    }
}

impl From<AccountNotificationConstraints> for Error<'static> {
    fn from(constraint: AccountNotificationConstraints) -> Self {
        let error = match constraint {
            AccountNotificationConstraints::ParamsSize => ErrorKind::BadRequest
                .with_message("Notification params must be between 2 and 4096 bytes"),
        };

        error.with_resource("notification")
    }
}
