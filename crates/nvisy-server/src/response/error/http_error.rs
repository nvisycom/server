//! HTTP error handling with builder pattern for dynamic error responses.
//!
//! This module provides comprehensive HTTP error handling with a builder pattern
//! that allows for dynamic error messages and resource-specific context.

use std::borrow::Cow;
use std::fmt;

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use strum::EnumIter;

use super::ErrorResponse;

/// The error type for HTTP handlers in the server.
///
/// This error type provides a comprehensive way to handle HTTP errors with proper
/// status codes, messages, and optional context information.
#[derive(Clone)]
#[must_use = "errors do nothing unless serialized"]
pub struct Error<'a> {
    /// The error category, which determines the HTTP status and default message.
    pub kind: ErrorKind,
    /// The resource the error relates to, if any.
    pub resource: Option<Cow<'a, str>>,
    /// Debugging context appended to the response, if any.
    pub context: Option<Cow<'a, str>>,
    /// A custom user-facing message overriding the kind's default, if any.
    pub message: Option<Cow<'a, str>>,
}

impl Error<'static> {
    /// Creates a new [`Error`] with the specified kind.
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            resource: None,
            context: None,
            message: None,
        }
    }

    /// Creates a [`NotFound`](ErrorKind::NotFound) error for the given resource.
    pub fn not_found(resource: &'static str) -> Self {
        Self::new(ErrorKind::NotFound)
            .with_message(format!("{resource} not found"))
            .with_resource(resource)
    }
}

impl<'a> Error<'a> {
    /// Attaches context information to the error.
    ///
    /// Context provides additional information about what went wrong,
    /// which will be included in the error response for debugging.
    #[inline]
    pub fn with_context(self, context: impl Into<Cow<'a, str>>) -> Self {
        Self {
            context: Some(context.into()),
            ..self
        }
    }

    /// Sets a custom user-friendly message for the error.
    #[inline]
    pub fn with_message(self, message: impl Into<Cow<'a, str>>) -> Self {
        Self {
            message: Some(message.into()),
            ..self
        }
    }

    /// Sets the resource that caused the error.
    #[inline]
    pub fn with_resource(self, resource: impl Into<Cow<'a, str>>) -> Self {
        Self {
            resource: Some(resource.into()),
            ..self
        }
    }

    /// Returns the error kind.
    ///
    /// A convenience over the public [`kind`](Self::kind) field for the common
    /// `error.kind() == ErrorKind::X` check, returning it by copy from `&self`.
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Converts this error into a static version by cloning all borrowed data.
    pub fn into_owned(self) -> Error<'static> {
        Error {
            kind: self.kind,
            context: self.context.map(|c| Cow::Owned(c.into_owned())),
            message: self.message.map(|m| Cow::Owned(m.into_owned())),
            resource: self.resource.map(|r| Cow::Owned(r.into_owned())),
        }
    }
}

impl Default for Error<'static> {
    #[inline]
    fn default() -> Self {
        Self {
            kind: ErrorKind::default(),
            context: None,
            message: None,
            resource: None,
        }
    }
}

impl fmt::Debug for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let response = self.kind.response();

        let mut debug_struct = f.debug_struct("Error");
        debug_struct
            .field("kind", &self.kind)
            .field("name", &response.name)
            .field("status", &response.status)
            .field("message", &response.message)
            .field("resource", &response.resource);

        if let Some(ref context) = self.context {
            debug_struct.field("context", context);
        }

        if let Some(ref message) = self.message {
            debug_struct.field("custom_message", message);
        }

        if let Some(ref resource) = self.resource {
            debug_struct.field("custom_resource", resource);
        }

        debug_struct.finish()
    }
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let response = self.kind.response();
        let message = self.message.as_deref().unwrap_or("Unknown error");

        write!(f, "{} ({}): {}", response.name, response.status, message)?;

        if let Some(ref context) = self.context {
            write!(f, " - {}", context)?;
        }

        if let Some(ref resource) = self.resource {
            write!(f, " [resource: {}]", resource)?;
        }

        Ok(())
    }
}

impl std::error::Error for Error<'_> {}

impl IntoResponse for Error<'_> {
    fn into_response(self) -> Response {
        // The kind supplies the defaults (name, status, and fallback message);
        // this error's own fields override the message and add the per-occurrence
        // resource and context.
        let defaults = self.kind.response();
        ErrorResponse {
            name: defaults.name,
            message: self.message.unwrap_or(defaults.message),
            resource: self.resource,
            context: self.context,
            status: defaults.status,
        }
        .into_response()
    }
}

impl From<ErrorKind> for Error<'static> {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}

/// A specialized [`Result`] type for HTTP operations.
///
/// This is the standard result type used throughout the nvisy server
/// for operations that can fail with an HTTP error.
///
/// [`Result`]: std::result::Result
pub type Result<T, E = Error<'static>> = std::result::Result<T, E>;

/// Comprehensive enumeration of all possible HTTP error kinds.
///
/// Each variant corresponds to a specific HTTP status code and error scenario.
/// The variants are organized by HTTP status code family.
#[must_use = "error kinds do nothing unless used to create errors"]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum ErrorKind {
    // 4xx Client Errors
    /// 400 Bad Request - Missing required path parameter
    MissingPathParam,
    /// 400 Bad Request - Invalid request data
    BadRequest,
    /// 401 Unauthorized - Missing authentication token
    MissingAuthToken,
    /// 401 Unauthorized - Malformed authentication token
    MalformedAuthToken,
    /// 401 Unauthorized - Invalid credentials
    Unauthorized,
    /// 403 Forbidden - Access denied
    Forbidden,
    /// 404 Not Found - Resource not found
    NotFound,
    /// 409 Conflict - Conflicting resource state
    Conflict,
    /// 413 Payload Too Large - Request body exceeds the allowed size
    PayloadTooLarge,
    /// 429 Too Many Requests - Rate limit exceeded
    TooManyRequests,

    // 5xx Server Errors
    /// 500 Internal Server Error - Unexpected server error
    #[default]
    InternalServerError,
    /// 501 Not Implemented - Feature not yet implemented
    NotImplemented,
    /// 503 Service Unavailable - The server is temporarily overloaded
    ServiceUnavailable,
}

impl ErrorKind {
    /// Converts this error kind into a full [`Error`].
    #[inline]
    pub fn into_error(self) -> Error<'static> {
        Error::new(self)
    }

    /// Creates an [`Error`] with the specified context.
    ///
    /// This is a convenience method for creating contextual errors.
    #[inline]
    pub fn with_context<'a>(self, context: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_context(context)
    }

    /// Creates an [`Error`] with the specified message.
    ///
    /// This is a convenience method for creating errors with custom messages.
    #[inline]
    pub fn with_message<'a>(self, message: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_message(message)
    }

    /// Creates an [`Error`] with the specified resource.
    ///
    /// This is a convenience method for creating resource-specific errors.
    #[inline]
    pub fn with_resource<'a>(self, resource: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_resource(resource)
    }

    /// Returns the default [`ErrorResponse`] for this kind: its machine-readable
    /// name, HTTP status, and default user-facing message.
    ///
    /// This match is the single source of truth for each variant's wire
    /// metadata — a new variant is described in exactly one place — and
    /// [`status_code`](Self::status_code) reads its status from here.
    #[inline]
    pub const fn response(self) -> ErrorResponse<'static> {
        match self {
            Self::MissingPathParam => ErrorResponse::new(
                "missing_path_param",
                "Missing path parameter",
                StatusCode::BAD_REQUEST,
            ),
            Self::BadRequest => ErrorResponse::new(
                "bad_request",
                "Invalid request data",
                StatusCode::BAD_REQUEST,
            ),
            Self::MissingAuthToken => ErrorResponse::new(
                "missing_auth_token",
                "Missing auth token",
                StatusCode::UNAUTHORIZED,
            ),
            Self::MalformedAuthToken => ErrorResponse::new(
                "malformed_auth_token",
                "Malformed auth token",
                StatusCode::UNAUTHORIZED,
            ),
            Self::Unauthorized => ErrorResponse::new(
                "unauthorized",
                "Invalid credentials",
                StatusCode::UNAUTHORIZED,
            ),
            Self::Forbidden => {
                ErrorResponse::new("forbidden", "Resource access denied", StatusCode::FORBIDDEN)
            }
            Self::NotFound => {
                ErrorResponse::new("not_found", "Resource not found", StatusCode::NOT_FOUND)
            }
            Self::Conflict => {
                ErrorResponse::new("conflict", "Resource state conflict", StatusCode::CONFLICT)
            }
            Self::PayloadTooLarge => ErrorResponse::new(
                "payload_too_large",
                "Payload too large",
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
            Self::TooManyRequests => ErrorResponse::new(
                "too_many_requests",
                "Rate limit exceeded",
                StatusCode::TOO_MANY_REQUESTS,
            ),
            Self::InternalServerError => ErrorResponse::new(
                "internal_server_error",
                "Internal server error",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::NotImplemented => ErrorResponse::new(
                "not_implemented",
                "Not implemented",
                StatusCode::NOT_IMPLEMENTED,
            ),
            Self::ServiceUnavailable => ErrorResponse::new(
                "service_unavailable",
                "Service unavailable",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
        }
    }

    /// Returns the HTTP status code for this error kind.
    #[inline]
    pub fn status_code(self) -> StatusCode {
        self.response().status
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.response().name.as_ref())
    }
}

impl IntoResponse for ErrorKind {
    #[inline]
    fn into_response(self) -> Response {
        self.response().into_response()
    }
}

impl<'a> OperationOutput for Error<'a> {
    type Inner = ErrorResponse<'static>;

    fn operation_response(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Option<aide::openapi::Response> {
        axum::Json::<ErrorResponse<'static>>::operation_response(ctx, operation)
    }

    fn inferred_responses(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        // Return empty vec to prevent aide from adding a default 200 response.
        // Error responses should be explicitly documented via TransformOperation.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_http_error() {
        let error = Error::default();
        assert_eq!(error.kind(), ErrorKind::InternalServerError);
        let _ = error.into_response();
    }

    #[test]
    fn error_from_kind() {
        let error = Error::new(ErrorKind::NotFound);
        assert_eq!(error.kind(), ErrorKind::NotFound);
        let _ = error.into_response();
    }

    #[test]
    fn error_with_context() {
        let error = ErrorKind::BadRequest.with_context("Invalid format");
        assert_eq!(error.context.as_deref(), Some("Invalid format"));
        let _ = error.into_response();
    }

    #[test]
    fn error_with_message() {
        let error = ErrorKind::NotFound.with_message("Custom not found message");
        assert_eq!(error.message.as_deref(), Some("Custom not found message"));
        let _ = error.into_response();
    }

    #[test]
    fn error_with_resource() {
        let error = ErrorKind::Forbidden.with_resource("document");
        assert_eq!(error.resource.as_deref(), Some("document"));
        let _ = error.into_response();
    }

    #[test]
    fn error_builder_chaining() {
        let error = ErrorKind::NotFound
            .with_message("Document not found")
            .with_resource("document")
            .with_context("ID: 123");

        assert_eq!(error.kind, ErrorKind::NotFound);
        assert_eq!(error.message.as_deref(), Some("Document not found"));
        assert_eq!(error.resource.as_deref(), Some("document"));
        assert_eq!(error.context.as_deref(), Some("ID: 123"));
    }

    #[test]
    fn std_fmt_display() {
        let error = ErrorKind::NotFound
            .with_message("Resource not found")
            .with_resource("document")
            .with_context("ID: 123");

        let display = format!("{}", error);
        assert!(display.contains("not_found"));
        assert!(display.contains("404"));
        assert!(display.contains("Resource not found"));
        assert!(display.contains("ID: 123"));
        assert!(display.contains("document"));
    }

    #[test]
    fn std_fmt_debug() {
        let error = ErrorKind::Forbidden
            .with_message("Access denied")
            .with_resource("document")
            .with_context("User lacks permissions");

        let debug = format!("{:?}", error);
        assert!(debug.contains("Forbidden"));
        assert!(debug.contains("Access denied"));
        assert!(debug.contains("document"));
    }

    #[test]
    fn std_error_trait() {
        let error = Error::new(ErrorKind::BadRequest);
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn error_into_static() {
        let error = ErrorKind::NotFound
            .with_message("Test message".to_string())
            .with_resource("test_resource".to_string())
            .with_context("Test context".to_string());

        let static_error = error.into_owned();
        assert_eq!(static_error.message.as_deref(), Some("Test message"));
        assert_eq!(static_error.resource.as_deref(), Some("test_resource"));
        assert_eq!(static_error.context.as_deref(), Some("Test context"));
    }

    #[test]
    fn every_error_kind_has_a_client_or_server_response() {
        use strum::IntoEnumIterator;

        // Iterating the variants (rather than a hand-kept list) means a newly
        // added `ErrorKind` is covered here automatically.
        for kind in ErrorKind::iter() {
            let response = kind.response();
            assert!(!response.name.is_empty(), "{kind:?} has an empty name");
            assert!(
                response.status.as_u16() >= 400,
                "{kind:?} maps to a non-error status"
            );
            let _ = kind.into_response();
        }
    }
}
