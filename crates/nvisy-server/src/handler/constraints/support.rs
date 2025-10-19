//! Support-related constraint violation error handlers.

use nvisy_postgres::types::{
    FeedbackConstraints, SupportTicketConstraints, SupportTicketReplyConstraints,
};

use crate::handler::{Error, ErrorKind};

impl From<SupportTicketConstraints> for Error<'static> {
    fn from(c: SupportTicketConstraints) -> Self {
        match c {
            SupportTicketConstraints::TicketNumberNotEmpty => {
                ErrorKind::InternalServerError.into_error()
            }
            SupportTicketConstraints::SubjectLengthMin => ErrorKind::BadRequest
                .with_context("Ticket subject must be at least 3 characters long"),
            SupportTicketConstraints::SubjectLengthMax => {
                ErrorKind::BadRequest.with_context("Ticket subject is too long")
            }
            SupportTicketConstraints::SubjectNotEmpty => {
                ErrorKind::BadRequest.with_context("Ticket subject cannot be empty")
            }
            SupportTicketConstraints::DescriptionLengthMax => {
                ErrorKind::BadRequest.with_context("Ticket description is too long")
            }
            SupportTicketConstraints::ContactEmailFormat => {
                ErrorKind::BadRequest.with_context("Invalid contact email format")
            }
            SupportTicketConstraints::ContactNameLengthMin => {
                ErrorKind::BadRequest.with_context("Contact name is too short")
            }
            SupportTicketConstraints::ContactNameLengthMax => {
                ErrorKind::BadRequest.with_context("Contact name is too long")
            }
            SupportTicketConstraints::BrowserInfoSizeMax
            | SupportTicketConstraints::DeviceInfoSizeMax
            | SupportTicketConstraints::ErrorDetailsSizeMax => {
                ErrorKind::BadRequest.with_context("Support information is too large")
            }
            SupportTicketConstraints::TagsCountMax | SupportTicketConstraints::LabelsCountMax => {
                ErrorKind::BadRequest.with_context("Too many tags or labels")
            }
            SupportTicketConstraints::UpdatedAfterCreated
            | SupportTicketConstraints::DeletedAfterCreated
            | SupportTicketConstraints::FirstResponseAfterCreated
            | SupportTicketConstraints::ResolvedAfterCreated => {
                ErrorKind::InternalServerError.into_error()
            }
            SupportTicketConstraints::ResolvedStatusConsistency => {
                ErrorKind::InternalServerError.into_error()
            }
            SupportTicketConstraints::TicketNumberUnique => {
                ErrorKind::Conflict.with_context("Ticket number already exists")
            }
        }
    }
}

impl From<SupportTicketReplyConstraints> for Error<'static> {
    fn from(_c: SupportTicketReplyConstraints) -> Self {
        // Generic fallback for all reply constraints
        ErrorKind::InternalServerError.into_error()
    }
}

impl From<FeedbackConstraints> for Error<'static> {
    fn from(_c: FeedbackConstraints) -> Self {
        // Generic fallback for all feedback constraints
        ErrorKind::InternalServerError.into_error()
    }
}
