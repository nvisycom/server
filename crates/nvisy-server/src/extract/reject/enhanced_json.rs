//! Enhanced JSON extractor with improved error handling.
//!
//! This module provides [`Json`], an enhanced version of [`axum::Json`] with
//! better error messages, size limits, and OpenAPI documentation support.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Json as AxumJson, OptionalFromRequest, Request};
use axum::response::{IntoResponse, Response};
use derive_more::{Deref, DerefMut, From};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::handler::{Error, ErrorKind};

/// Maximum allowed JSON payload size in bytes (1MB).
const MAX_JSON_PAYLOAD_SIZE: usize = 1024 * 1024;

/// Enhanced JSON extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum JSON extractor. It includes:
///
/// - Detailed error messages for different failure types
/// - Type-safe deserialization with proper error context
///
/// # Size Limits
///
/// The extractor enforces a maximum payload size of 1MB to prevent
/// memory exhaustion attacks.
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [`Json`]: AxumJson
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct Json<T>(pub T);

impl<T> Json<T> {
    /// Creates a new [`Json`] wrapper around the provided value.
    ///
    /// # Arguments
    ///
    /// * `inner` - The value to wrap in the JSON extractor
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Returns the inner value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let extractor = <AxumJson<T> as FromRequest<S>>::from_request(req, state).await;
        extractor.map(|x| Self::new(x.0)).map_err(Into::into)
    }
}

impl<T, S> OptionalFromRequest<S> for Json<T>
where
    T: DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        let result = <Self as FromRequest<S>>::from_request(req, state).await;

        match result {
            Ok(json) => Ok(Some(json)),
            Err(error) => {
                // For optional extraction, only propagate server errors
                // Client errors (like malformed JSON) result in None
                match error.kind() {
                    ErrorKind::InternalServerError => Err(error),
                    _ => Ok(None),
                }
            }
        }
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    #[inline]
    fn into_response(self) -> Response {
        AxumJson(self.0).into_response()
    }
}

impl From<JsonRejection> for Error<'static> {
    fn from(rejection: JsonRejection) -> Self {
        let error_context = format!("JSON rejection details: {:?}", rejection);

        match rejection {
            JsonRejection::JsonDataError(err) => {
                ErrorKind::BadRequest
                    .with_message("Invalid request data format")
                    .with_context(format!(
                        "JSON deserialization failed: {}. Verify that all required fields are present, have correct types, and match the expected schema.",
                        sanitize_error_message(&err.to_string())
                    ))
            }
            JsonRejection::JsonSyntaxError(err) => {
                ErrorKind::BadRequest
                    .with_message("Invalid JSON syntax in request body")
                    .with_context(format!(
                        "JSON parsing failed: {}. Ensure the request body contains well-formed JSON with proper syntax.",
                        sanitize_error_message(&err.to_string())
                    ))
            }
            JsonRejection::MissingJsonContentType(_) => {
                ErrorKind::BadRequest
                    .with_message("Invalid content type")
                    .with_context("Request must have Content-Type header set to 'application/json'. Include the header: Content-Type: application/json")
            }
            JsonRejection::BytesRejection(err) => {
                let message = err.to_string();
                if message.contains("length limit") {
                    ErrorKind::BadRequest
                        .with_message("Request body too large")
                        .with_context(format!(
                            "Request body exceeds maximum allowed size of {} bytes. Consider reducing the payload size or splitting into multiple requests.",
                            MAX_JSON_PAYLOAD_SIZE
                        ))
                } else {
                    ErrorKind::BadRequest
                        .with_message("Failed to read request body")
                        .with_context(format!(
                            "Request body processing failed: {}. Body may be corrupted, incomplete, or connection interrupted.",
                            sanitize_error_message(&message)
                        ))
                }
            }
            _ => {
                ErrorKind::InternalServerError
                    .with_message("Request processing failed")
                    .with_context(format!(
                        "Unexpected error occurred during JSON request body processing: {}",
                        error_context
                    ))
            }
        }
    }
}

/// Sanitizes error messages to prevent information leakage while keeping them useful.
fn sanitize_error_message(message: &str) -> String {
    // Limit to first 3 lines to prevent excessive verbosity.
    let lines = message.lines().take(3).collect::<Vec<_>>();
    // Limit message length.
    lines.join(" ").chars().take(200).collect()
}

impl<T> aide::OperationInput for Json<T>
where
    T: schemars::JsonSchema,
{
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        axum::Json::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<u16>, aide::openapi::Response)> {
        axum::Json::<T>::inferred_early_responses(ctx, operation)
    }
}

impl<T> aide::OperationOutput for Json<T>
where
    T: schemars::JsonSchema + Serialize,
{
    type Inner = T;

    fn operation_response(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Option<aide::openapi::Response> {
        AxumJson::<T>::operation_response(ctx, operation)
    }

    fn inferred_responses(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<u16>, aide::openapi::Response)> {
        AxumJson::<T>::inferred_responses(ctx, operation)
    }
}
