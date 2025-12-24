use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, OptionalFromRequestParts, Path as AxumPath};
use axum::http::request::Parts;
use derive_more::{Deref, DerefMut, From};
use serde::de::DeserializeOwned;

use crate::handler::{Error, ErrorKind};

/// Enhanced path parameter extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum [`Path`] extractor. It includes:
///
/// - Detailed error messages for different parameter types
/// - Type-safe deserialization with proper error context
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [`Path`]: AxumPath
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct Path<T>(pub T);

impl<T> Path<T> {
    /// Creates a new instance of [`Path`].
    ///
    /// # Arguments
    ///
    /// * `inner` - The deserialized path parameters
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Returns the inner path parameters.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let extractor =
            <AxumPath<T> as FromRequestParts<S>>::from_request_parts(parts, state).await;
        extractor.map(|x| Self(x.0)).map_err(Into::into)
    }
}

impl<T, S> OptionalFromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let extractor =
            <AxumPath<T> as OptionalFromRequestParts<S>>::from_request_parts(parts, state).await;

        match extractor {
            Ok(maybe_path) => Ok(maybe_path.map(|x| Self::new(x.0))),
            Err(rejection) => {
                // For optional extraction, only propagate server errors
                match rejection {
                    PathRejection::FailedToDeserializePathParams(_)
                    | PathRejection::MissingPathParams(_) => Ok(None),
                    _ => Err(rejection.into()),
                }
            }
        }
    }
}

impl From<PathRejection> for Error<'static> {
    fn from(rejection: PathRejection) -> Self {
        match rejection {
            PathRejection::FailedToDeserializePathParams(err) => {
                let error_message = err.to_string();
                let enhanced_context = enhance_deserialization_error(&error_message);


                ErrorKind::BadRequest
                    .with_message("Invalid path parameter format")
                    .with_context(format!(
                        "Path parameter deserialization failed: {}. {}",
                        sanitize_error_message(&error_message),
                        enhanced_context
                    ))
            }
            PathRejection::MissingPathParams(err) => {
                let error_message = err.to_string();


                ErrorKind::MissingPathParam
                    .with_message("Required path parameter missing")
                    .with_context(format!(
                        "Path parameter extraction failed: {}. Ensure all required parameters are present in the URL path and match the expected route pattern.",
                        sanitize_error_message(&error_message)
                    ))
            }
            _ => {

                ErrorKind::InternalServerError
                    .with_message("Path processing failed")
                    .with_context("Unexpected error occurred during path parameter processing. This may indicate a routing configuration issue.")
            }
        }
    }
}

/// Enhances deserialization error messages with type-specific guidance.
fn enhance_deserialization_error(error_message: &str) -> &'static str {
    let error_lower = error_message.to_lowercase();

    if error_lower.contains("uuid") || error_lower.contains("invalid character") {
        "UUID parameters must be in format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (32 hexadecimal digits with hyphens)"
    } else if error_lower.contains("invalid digit") || error_lower.contains("cannot parse") {
        "Numeric parameters must contain only digits and be within the valid range for the expected type"
    } else if error_lower.contains("bool") {
        "Boolean parameters must be 'true' or 'false'"
    } else if error_lower.contains("enum") {
        "Enum parameters must match one of the defined variants exactly"
    } else {
        "Check that the parameter format matches the expected type definition"
    }
}

/// Sanitizes error messages to prevent information leakage while keeping them useful.
fn sanitize_error_message(message: &str) -> String {
    // Remove potentially sensitive information while keeping the core error message
    message
        .lines()
        .take(2) // Limit to first 2 lines to prevent excessive verbosity
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(150) // Limit message length
        .collect()
}

impl<T> aide::OperationInput for Path<T>
where
    T: schemars::JsonSchema,
{
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        AxumPath::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<u16>, aide::openapi::Response)> {
        AxumPath::<T>::inferred_early_responses(ctx, operation)
    }
}
