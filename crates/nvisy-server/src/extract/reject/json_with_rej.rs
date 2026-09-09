//! Enhanced JSON extractor with improved error handling.
//!
//! This module provides [`Json`], an enhanced version of [`axum::Json`] with
//! better error messages, size limits, and OpenAPI documentation support.

use aide::generate::GenContext;
use aide::openapi::Operation;
use aide::{OperationInput, OperationOutput};
use axum::extract::rejection::{BytesRejection, FailedToBufferBody, JsonRejection};
use axum::extract::{FromRequest, Json as AxumJson, OptionalFromRequest, Request};
use axum::response::{IntoResponse, Response};
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::sanitize_error_message;
use crate::handler::{Error, ErrorKind};

/// Enhanced JSON extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum JSON extractor. It includes:
///
/// - Detailed error messages for different failure types
/// - Type-safe deserialization with proper error context
///
/// The [`FromRequest`] impl is derived: extraction delegates to [`axum::Json`]
/// and its [`JsonRejection`] is mapped into our [`Error`] by the `From` impl
/// below (that mapping is where the improved messages live).
///
/// # Size Limits
///
/// Request-body size limits apply via the router's body-limit layer; a body
/// that exceeds it is rejected before deserialization.
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [`Json`]: AxumJson
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From, FromRequest)]
#[from_request(via(AxumJson), rejection(Error<'static>))]
pub struct Json<T>(pub T);

impl<T, S> OptionalFromRequest<S> for Json<T>
where
    T: DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequest<S>>::from_request(req, state).await {
            Ok(json) => Ok(Some(json)),
            // Only a server error is worth surfacing; a malformed or absent body
            // is a legitimately empty optional.
            Err(error) if error.kind() == ErrorKind::InternalServerError => Err(error),
            Err(_) => Ok(None),
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
            // A body that trips the router's size limit is a distinct, typed
            // variant — match it rather than sniffing the Display string.
            JsonRejection::BytesRejection(BytesRejection::FailedToBufferBody(
                FailedToBufferBody::LengthLimitError(_),
            )) => ErrorKind::BadRequest
                .with_message("Request body too large")
                .with_context(
                    "Request body exceeds the maximum allowed size. Consider reducing the payload size or splitting into multiple requests.",
                ),
            JsonRejection::BytesRejection(err) => ErrorKind::BadRequest
                .with_message("Failed to read request body")
                .with_context(format!(
                    "Request body processing failed: {}. Body may be corrupted, incomplete, or connection interrupted.",
                    sanitize_error_message(&err.to_string())
                )),
            // `JsonRejection` is `#[non_exhaustive]`; a future variant lands here.
            _ => ErrorKind::BadRequest
                .with_message("Invalid JSON request body")
                .with_context("The request body could not be processed as JSON."),
        }
    }
}

impl<T> OperationInput for Json<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        axum::Json::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        axum::Json::<T>::inferred_early_responses(ctx, operation)
    }
}

impl<T> OperationOutput for Json<T>
where
    T: JsonSchema + Serialize,
{
    type Inner = T;

    fn operation_response(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Option<aide::openapi::Response> {
        AxumJson::<T>::operation_response(ctx, operation)
    }

    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        AxumJson::<T>::inferred_responses(ctx, operation)
    }
}
