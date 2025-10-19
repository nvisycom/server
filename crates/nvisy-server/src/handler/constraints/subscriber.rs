//! Subscriber-related constraint violation error handlers.

use nvisy_postgres::types::SubscriberConstraints;

use crate::handler::{Error, ErrorKind};

impl From<SubscriberConstraints> for Error<'static> {
    fn from(c: SubscriberConstraints) -> Self {
        match c {
            SubscriberConstraints::EmailFormat => {
                ErrorKind::BadRequest.with_context("Invalid email format")
            }
            SubscriberConstraints::EmailLengthMax => {
                ErrorKind::BadRequest.with_context("Email address is too long")
            }
            SubscriberConstraints::FirstNameLengthMax
            | SubscriberConstraints::LastNameLengthMax => {
                ErrorKind::BadRequest.with_context("Name is too long")
            }
            SubscriberConstraints::UnsubscribeReasonLengthMax => {
                ErrorKind::BadRequest.with_context("Unsubscribe reason is too long")
            }
            SubscriberConstraints::VerificationTokenLengthMin
            | SubscriberConstraints::EngagementCountsMin
            | SubscriberConstraints::EngagementScoreRange
            | SubscriberConstraints::BounceCountMin
            | SubscriberConstraints::ComplaintCountMin => {
                ErrorKind::InternalServerError.into_error()
            }
            SubscriberConstraints::UpdatedAfterCreated
            | SubscriberConstraints::DeletedAfterCreated
            | SubscriberConstraints::ConfirmedAfterCreated
            | SubscriberConstraints::UnsubscribedAfterCreated
            | SubscriberConstraints::LastOpenedAfterCreated
            | SubscriberConstraints::LastClickedAfterCreated
            | SubscriberConstraints::LastBouncedAfterCreated
            | SubscriberConstraints::LastComplaintAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            SubscriberConstraints::VerificationExpiresFuture
            | SubscriberConstraints::ConfirmedStatusConsistency
            | SubscriberConstraints::UnsubscribedStatusConsistency
            | SubscriberConstraints::VerificationTokenConsistency => {
                ErrorKind::InternalServerError.into_error()
            }
            SubscriberConstraints::EmailAddressUnique => {
                ErrorKind::Conflict.with_context("An account with this email already exists")
            }
        }
    }
}
