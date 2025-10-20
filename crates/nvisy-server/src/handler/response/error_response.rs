use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// HTTP error response representation with security-conscious design.
///
/// This struct contains all the information needed to serialize an error
/// response, including the error name, message, HTTP status code, resource
/// information, and user-friendly messages.
#[must_use = "error responses do nothing unless serialized"]
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse<'a> {
    /// The error name/type identifier
    pub name: Cow<'a, str>,
    /// User-friendly error message safe for client display
    pub message: Cow<'a, str>,
    /// The resource that the error relates to (optional, set by handler)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Cow<'a, str>>,
    /// Internal context for debugging (optional, not exposed to client)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Cow<'a, str>>,
    /// HTTP status code (not serialized in JSON)
    #[serde(skip)]
    pub status: StatusCode,
}

impl<'a> ErrorResponse<'a> {
    // 4xx Client Errors
    pub const BAD_REQUEST: Self = Self::new(
        "bad_request",
        "The request could not be processed due to invalid data",
        StatusCode::BAD_REQUEST,
    );
    pub const CONFLICT: Self = Self::new(
        "conflict",
        "The request conflicts with the current state of the resource",
        StatusCode::CONFLICT,
    );
    pub const FORBIDDEN: Self = Self::new(
        "forbidden",
        "You don't have permission to access this resource",
        StatusCode::FORBIDDEN,
    );
    // 5xx Server Errors
    pub const INTERNAL_SERVER_ERROR: Self = Self::new(
        "internal_server_error",
        "An internal server error occurred. Please try again later",
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    pub const MALFORMED_AUTH_TOKEN: Self = Self::new(
        "malformed_auth_token",
        "The authentication token format is invalid",
        StatusCode::UNAUTHORIZED,
    );
    pub const MISSING_AUTH_TOKEN: Self = Self::new(
        "missing_auth_token",
        "Authentication is required to access this resource",
        StatusCode::UNAUTHORIZED,
    );
    pub const MISSING_PATH_PARAM: Self = Self::new(
        "missing_path_param",
        "Invalid request: missing required parameters",
        StatusCode::BAD_REQUEST,
    );
    pub const NOT_FOUND: Self = Self::new(
        "not_found",
        "The requested resource was not found",
        StatusCode::NOT_FOUND,
    );
    pub const NOT_IMPLEMENTED: Self = Self::new(
        "not_implemented",
        "This feature is not yet available",
        StatusCode::NOT_IMPLEMENTED,
    );
    pub const TOO_MANY_REQUESTS: Self = Self::new(
        "too_many_requests",
        "Too many requests. Please slow down and try again later",
        StatusCode::TOO_MANY_REQUESTS,
    );
    pub const UNAUTHORIZED: Self = Self::new(
        "unauthorized",
        "Invalid or expired authentication credentials",
        StatusCode::UNAUTHORIZED,
    );

    /// Creates a new error response.
    #[inline]
    pub const fn new(name: &'a str, message: &'a str, status: StatusCode) -> Self {
        Self {
            name: Cow::Borrowed(name),
            message: Cow::Borrowed(message),
            resource: None,
            context: None,
            status,
        }
    }

    /// Creates a new error response with custom resource.
    /// If a resource already exists, it merges them with a separator.
    pub fn with_resource(mut self, resource: impl Into<Cow<'a, str>>) -> Self {
        let new_resource = resource.into();
        self.resource = Some(match self.resource {
            Some(existing) => Cow::Owned(format!("{}/{}", existing, new_resource)),
            None => new_resource,
        });
        self
    }

    /// Creates a new error response with custom message.
    /// Appends the new message to the existing message.
    pub fn with_message(mut self, message: impl Into<Cow<'a, str>>) -> Self {
        let new_message = message.into();
        self.message = Cow::Owned(format!("{}. {}", self.message, new_message));
        self
    }

    /// Attaches context to the error response.
    /// If context already exists, it merges them with a separator.
    pub fn with_context(mut self, context: impl Into<Cow<'a, str>>) -> Self {
        let new_context = context.into();
        self.context = Some(match self.context {
            Some(existing) => Cow::Owned(format!("{}; {}", existing, new_context)),
            None => new_context,
        });
        self
    }
}

impl Default for ErrorResponse<'_> {
    #[inline]
    fn default() -> Self {
        Self::INTERNAL_SERVER_ERROR
    }
}

impl IntoResponse for ErrorResponse<'_> {
    #[inline]
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_merging_resource() {
        let response = ErrorResponse::NOT_FOUND
            .with_resource("project")
            .with_resource("document");

        assert_eq!(response.resource.as_deref(), Some("project/document"));
    }

    #[test]
    fn error_response_merging_message() {
        let response = ErrorResponse::BAD_REQUEST
            .with_message("Invalid format")
            .with_message("Missing required field");

        assert_eq!(
            &response.message,
            "The request could not be processed due to invalid data. Invalid format. Missing required field"
        );
    }

    #[test]
    fn error_response_merging_context() {
        let response = ErrorResponse::INTERNAL_SERVER_ERROR
            .with_context("Database connection failed")
            .with_context("Retry attempted 3 times");

        assert_eq!(
            response.context.as_deref(),
            Some("Database connection failed; Retry attempted 3 times")
        );
    }

    #[test]
    fn error_response_serialization() {
        let response = ErrorResponse::BAD_REQUEST
            .with_resource("test_resource")
            .with_message("Test message")
            .with_context("Test context");

        let json = serde_json::to_string(&response).unwrap();

        // Should contain all serialized fields
        assert!(json.contains("name"));
        assert!(json.contains("message"));
        assert!(json.contains("resource"));
        assert!(json.contains("context"));

        // Should not contain status code (marked as skip)
        assert!(!json.contains("status"));
    }
}
