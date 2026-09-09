//! Enhanced query parameter extractor with improved error handling.
//!
//! This module provides [`Query`], a query parameter extractor with detailed
//! error messages and OpenAPI documentation support. Repeated keys
//! (`?f=a&f=b`) deserialize into a sequence.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::FromRequestParts;
use axum_extra::extract::{Query as AxumQuery, QueryRejection};
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;

use super::sanitize_error_message;
use crate::handler::{Error, ErrorKind};

/// Enhanced query parameter extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum Query extractor. It includes:
///
/// - Detailed error messages for different parameter parsing failures
/// - Type-safe deserialization with proper error context
/// - Clear indication of which parameters failed validation
///
/// The [`FromRequestParts`] impl is derived: extraction delegates to
/// [`axum_extra::extract::Query`] and its [`QueryRejection`] is mapped into our
/// [`Error`] by the `From` impl below (that mapping is where the improved
/// messages live).
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging.
///
/// [`Query`]: AxumQuery
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From, FromRequestParts)]
#[from_request(via(AxumQuery), rejection(Error<'static>))]
pub struct Query<T>(pub T);

/// Maps a query rejection into a structured bad-request [`Error`].
///
/// The deserializer message is sanitized before it becomes context so that
/// submitted query values are not echoed back or logged.
impl From<QueryRejection> for Error<'static> {
    fn from(rejection: QueryRejection) -> Self {
        let context = sanitize_error_message(&rejection.to_string());

        tracing::debug!(
            target: "nvisy::extract::query",
            error = %context,
            "Query parameter parsing failed"
        );

        ErrorKind::BadRequest
            .with_message("Invalid query parameters")
            .with_context(context)
    }
}

impl<T> OperationInput for Query<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        <AxumQuery<T> as OperationInput>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        <AxumQuery<T> as OperationInput>::inferred_early_responses(ctx, operation)
    }
}
