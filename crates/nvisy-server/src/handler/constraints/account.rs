//! Account-related constraint violation error handlers.

use nvisy_postgres::types::{
    AccountConstraints, AccountSessionConstraints, AccountTokenConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<AccountConstraints> for Error<'static> {
    fn from(c: AccountConstraints) -> Self {
        match c {
            AccountConstraints::DisplayNameLengthMin => ErrorKind::BadRequest
                .with_context("Display name must be at least 2 characters long"),
            AccountConstraints::DisplayNameLengthMax => {
                ErrorKind::BadRequest.with_context("Display name cannot exceed 32 characters")
            }
            AccountConstraints::DisplayNameNotEmpty => {
                ErrorKind::BadRequest.with_context("Display name cannot be empty")
            }
            AccountConstraints::EmailFormat => {
                ErrorKind::BadRequest.with_context("Invalid email format")
            }
            AccountConstraints::EmailLengthMax => {
                ErrorKind::BadRequest.with_context("Email address is too long")
            }
            AccountConstraints::PasswordHashNotEmpty => {
                ErrorKind::BadRequest.with_context("Password cannot be empty")
            }
            AccountConstraints::PasswordHashLengthMin => {
                ErrorKind::BadRequest.with_context("Password hash is too short")
            }
            AccountConstraints::CompanyNameLengthMax => {
                ErrorKind::BadRequest.with_context("Company name is too long")
            }
            AccountConstraints::PhoneNumberLengthMax => {
                ErrorKind::BadRequest.with_context("Phone number is too long")
            }
            AccountConstraints::TimezoneFormat => {
                ErrorKind::BadRequest.with_context("Invalid timezone format")
            }
            AccountConstraints::LocaleFormat => {
                ErrorKind::BadRequest.with_context("Invalid locale format")
            }
            AccountConstraints::FailedLoginAttemptsMin => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountConstraints::FailedLoginAttemptsMax => {
                ErrorKind::BadRequest.with_context("Too many failed login attempts")
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
                ErrorKind::BadRequest.with_context("Admin accounts cannot be suspended")
            }
            AccountConstraints::EmailAddressUnique => ErrorKind::Conflict
                .with_context("An account with this email address already exists"),
        }
    }
}

impl From<AccountSessionConstraints> for Error<'static> {
    fn from(c: AccountSessionConstraints) -> Self {
        match c {
            AccountSessionConstraints::RegionCodeValid => {
                ErrorKind::BadRequest.with_context("Invalid region code")
            }
            AccountSessionConstraints::CountryCodeValid => {
                ErrorKind::BadRequest.with_context("Invalid country code")
            }
            AccountSessionConstraints::UserAgentNotEmpty => {
                ErrorKind::BadRequest.with_context("User agent cannot be empty")
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
        }
    }
}

impl From<AccountTokenConstraints> for Error<'static> {
    fn from(c: AccountTokenConstraints) -> Self {
        match c {
            AccountTokenConstraints::PrimaryKey => ErrorKind::InternalServerError.into_error(),
            AccountTokenConstraints::ActionDataSizeMin => {
                ErrorKind::BadRequest.with_context("Action data is too small")
            }
            AccountTokenConstraints::ActionDataSizeMax => {
                ErrorKind::BadRequest.with_context("Action data exceeds maximum allowed size")
            }
            AccountTokenConstraints::UserAgentNotEmpty => {
                ErrorKind::BadRequest.with_context("User agent cannot be empty")
            }
            AccountTokenConstraints::AttemptCountMin => ErrorKind::InternalServerError.into_error(),
            AccountTokenConstraints::AttemptCountMax => {
                ErrorKind::BadRequest.with_context("Maximum attempts exceeded")
            }
            AccountTokenConstraints::MaxAttemptsMin | AccountTokenConstraints::MaxAttemptsMax => {
                ErrorKind::InternalServerError.into_error()
            }
            AccountTokenConstraints::ExpiredAfterIssued
            | AccountTokenConstraints::UsedAfterIssued
            | AccountTokenConstraints::UsedBeforeExpired => {
                ErrorKind::InternalServerError.into_error()
            }
        }
    }
}
