//! Enhanced query parameter extractor with improved error handling.
//!
//! This module provides [`Query`], an enhanced version of [`axum::extract::Query`]
//! with better error messages and OpenAPI documentation support.

use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRequestParts, OptionalFromRequestParts, Query as AxumQuery};
use axum::http::request::Parts;
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

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
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging.
///
/// [`Query`]: AxumQuery
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct Query<T>(pub T);

impl<T> Query<T> {
    /// Creates a new instance of [`Query`].
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Returns the inner query parameters.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AxumQuery::<T>::from_request_parts(parts, state).await {
            Ok(AxumQuery(query)) => Ok(Query(query)),
            Err(rejection) => Err(enhance_query_error(rejection)),
        }
    }
}

impl<T, S> OptionalFromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match AxumQuery::<T>::from_request_parts(parts, state).await {
            Ok(AxumQuery(query)) => Ok(Some(Query(query))),
            Err(_) => Ok(None),
        }
    }
}

/// Enhances query parameter parsing errors with detailed context and user-friendly messages.
///
/// This function takes the raw Axum query rejection and converts it into a more
/// informative error that helps developers understand what went wrong with the
/// query parameter parsing.
fn enhance_query_error(rejection: QueryRejection) -> Error<'static> {
    tracing::debug!(
        target: "nvisy::extract::query",
        error = %rejection,
        "Query parameter parsing failed"
    );

    match rejection {
        QueryRejection::FailedToDeserializeQueryString(err) => {
            // Extract the inner serde_urlencoded error for more specific handling
            let error_message = err.to_string();

            if error_message.contains("missing field") {
                let field_name = extract_field_name_from_error(&error_message);
                ErrorKind::BadRequest
                    .with_message("Missing required query parameter")
                    .with_context(format!(
                        "The query parameter '{}' is required but was not provided",
                        field_name.unwrap_or("unknown")
                    ))
            } else if error_message.contains("invalid type") {
                ErrorKind::BadRequest
                    .with_message("Invalid query parameter type")
                    .with_context(format!(
                        "Failed to parse query parameter: {}. Please check the parameter format and try again",
                        error_message
                    ))
            } else if error_message.contains("duplicate field") {
                let field_name = extract_field_name_from_error(&error_message);
                ErrorKind::BadRequest
                    .with_message("Duplicate query parameter")
                    .with_context(format!(
                        "The query parameter '{}' was provided multiple times. Please provide it only once",
                        field_name.unwrap_or("unknown")
                    ))
            } else {
                ErrorKind::BadRequest
                    .with_message("Invalid query parameters")
                    .with_context(format!("Failed to parse query string: {}", error_message))
            }
        }
        _ => {
            // Fallback for other query rejection types
            ErrorKind::BadRequest
                .with_message("Invalid query parameters")
                .with_context("The query string could not be parsed. Please check your parameters and try again")
        }
    }
}

/// Attempts to extract the field name from a serde error message.
///
/// This is a best-effort function that tries to parse field names from
/// error messages to provide more helpful error context.
fn extract_field_name_from_error(error_message: &str) -> Option<&str> {
    // Try to extract field name from common serde error patterns
    if let Some(start) = error_message.find('`')
        && let Some(end) = error_message[start + 1..].find('`')
    {
        return Some(&error_message[start + 1..start + 1 + end]);
    }

    // Try alternative patterns
    if error_message.contains("field ")
        && let Some(start) = error_message.find("field ")
    {
        let field_part = &error_message[start + 6..];
        if let Some(end) = field_part.find(' ') {
            return Some(&field_part[..end]);
        }
    }

    None
}

impl<T> aide::OperationInput for Query<T>
where
    T: JsonSchema,
{
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        AxumQuery::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<u16>, aide::openapi::Response)> {
        AxumQuery::<T>::inferred_early_responses(ctx, operation)
    }
}
