use std::borrow::Cow;
use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use schemars::JsonSchema;
use validator::ValidationErrors;

/// Error category for better error handling and logging.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Authentication and authorization errors
    Authentication,
    /// Request validation errors
    Validation,
    /// Business logic errors
    Business,
    /// External service errors
    External,
    /// Internal system errors
    Internal,
    /// Rate limiting errors
    RateLimit,
    /// Resource not found errors
    NotFound,
    /// Permission denied errors
    Permission,
}

/// Validation error details for field-specific errors.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValidationErrorDetail {
    /// Field name that failed validation
    pub field: String,
    /// Error code for the validation failure
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Additional parameters related to the validation error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, serde_json::Value>>,
}

/// HTTP error response representation with security-conscious design.
///
/// This struct contains all the information needed to serialize an error
/// response, including the error name, message, HTTP status code, resource
/// information, and user-friendly messages.
#[must_use = "error responses do nothing unless serialized"]
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ErrorResponse<'a> {
    /// The error name/type identifier
    pub name: Cow<'a, str>,
    /// User-friendly error message safe for client display
    pub message: Cow<'a, str>,
    /// Error category for better categorization
    pub category: ErrorCategory,
    /// The resource that the error relates to (optional, set by handler)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Cow<'a, str>>,
    /// Internal context for debugging (optional, not exposed to client)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Cow<'a, str>>,
    /// Helpful suggestion for resolving the error (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Cow<'a, str>>,
    /// Validation error details for field-specific errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_errors: Option<Vec<ValidationErrorDetail>>,
    /// Error correlation ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Cow<'a, str>>,
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
        ErrorCategory::Validation,
    );
    pub const CONFLICT: Self = Self::new(
        "conflict",
        "The request conflicts with the current state of the resource",
        StatusCode::CONFLICT,
        ErrorCategory::Business,
    );
    pub const FORBIDDEN: Self = Self::new(
        "forbidden",
        "You don't have permission to access this resource",
        StatusCode::FORBIDDEN,
        ErrorCategory::Permission,
    );
    pub const GATEWAY_TIMEOUT: Self = Self::new(
        "gateway_timeout",
        "The request timed out. Please try again",
        StatusCode::GATEWAY_TIMEOUT,
        ErrorCategory::External,
    );
    // 5xx Server Errors
    pub const INTERNAL_SERVER_ERROR: Self = Self::new(
        "internal_server_error",
        "An internal server error occurred. Please try again later",
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCategory::Internal,
    );
    // Authentication Errors
    pub const MALFORMED_AUTH_TOKEN: Self = Self::new(
        "malformed_auth_token",
        "The authentication token format is invalid",
        StatusCode::UNAUTHORIZED,
        ErrorCategory::Authentication,
    );
    pub const MISSING_AUTH_TOKEN: Self = Self::new(
        "missing_auth_token",
        "Authentication is required to access this resource",
        StatusCode::UNAUTHORIZED,
        ErrorCategory::Authentication,
    );
    pub const MISSING_PATH_PARAM: Self = Self::new(
        "missing_path_param",
        "Invalid request: missing required parameters",
        StatusCode::BAD_REQUEST,
        ErrorCategory::Validation,
    );
    // Resource Errors
    pub const NOT_FOUND: Self = Self::new(
        "not_found",
        "The requested resource was not found",
        StatusCode::NOT_FOUND,
        ErrorCategory::NotFound,
    );
    // System Errors
    pub const NOT_IMPLEMENTED: Self = Self::new(
        "not_implemented",
        "This feature is not yet available",
        StatusCode::NOT_IMPLEMENTED,
        ErrorCategory::Internal,
    );
    pub const PAYLOAD_TOO_LARGE: Self = Self::new(
        "payload_too_large",
        "Request payload exceeds size limits",
        StatusCode::PAYLOAD_TOO_LARGE,
        ErrorCategory::Validation,
    );
    pub const SERVICE_UNAVAILABLE: Self = Self::new(
        "service_unavailable",
        "Service is temporarily unavailable. Please try again later",
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCategory::External,
    );
    pub const TOKEN_EXPIRED: Self = Self::new(
        "token_expired",
        "Authentication token has expired",
        StatusCode::UNAUTHORIZED,
        ErrorCategory::Authentication,
    );
    // Rate Limiting
    pub const TOO_MANY_REQUESTS: Self = Self::new(
        "too_many_requests",
        "Too many requests. Please slow down and try again later",
        StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::RateLimit,
    );
    pub const UNAUTHORIZED: Self = Self::new(
        "unauthorized",
        "Invalid or expired authentication credentials",
        StatusCode::UNAUTHORIZED,
        ErrorCategory::Authentication,
    );
    pub const UNSUPPORTED_MEDIA_TYPE: Self = Self::new(
        "unsupported_media_type",
        "The media type of the request is not supported",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ErrorCategory::Validation,
    );
    pub const VALIDATION_ERROR: Self = Self::new(
        "validation_error",
        "Request validation failed",
        StatusCode::BAD_REQUEST,
        ErrorCategory::Validation,
    );

    /// Creates a new error response.
    #[inline]
    pub const fn new(
        name: &'a str,
        message: &'a str,
        status: StatusCode,
        category: ErrorCategory,
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            message: Cow::Borrowed(message),
            category,
            resource: None,
            context: None,
            suggestion: None,
            validation_errors: None,
            correlation_id: None,
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

    /// Attaches a suggestion to the error response.
    /// If a suggestion already exists, it merges them with a separator.
    pub fn with_suggestion(mut self, suggestion: impl Into<Cow<'a, str>>) -> Self {
        let new_suggestion = suggestion.into();
        self.suggestion = Some(match self.suggestion {
            Some(existing) => Cow::Owned(format!("{}; {}", existing, new_suggestion)),
            None => new_suggestion,
        });
        self
    }

    /// Adds validation errors to the error response.
    pub fn with_validation_errors(mut self, errors: Vec<ValidationErrorDetail>) -> Self {
        self.validation_errors = Some(errors);
        self
    }

    /// Adds a correlation ID to the error response.
    pub fn with_correlation_id(mut self, correlation_id: impl Into<Cow<'a, str>>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Creates an error response from validator ValidationErrors.
    pub fn from_validation_errors(validation_errors: ValidationErrors) -> Self {
        let mut error_details = Vec::new();

        for (field, field_errors) in validation_errors.field_errors() {
            for error in field_errors {
                let mut params = HashMap::new();
                for (key, value) in &error.params {
                    params.insert(key.to_string(), value.clone());
                }

                error_details.push(ValidationErrorDetail {
                    field: field.to_string(),
                    code: error.code.to_string(),
                    message: error
                        .message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("Validation failed for field '{}'", field)),
                    params: if params.is_empty() {
                        None
                    } else {
                        Some(params)
                    },
                });
            }
        }

        Self::VALIDATION_ERROR.with_validation_errors(error_details)
    }
}

impl Default for ErrorResponse<'_> {
    #[inline]
    fn default() -> Self {
        Self::INTERNAL_SERVER_ERROR
    }
}

impl From<ValidationErrors> for ErrorResponse<'_> {
    fn from(errors: ValidationErrors) -> Self {
        Self::from_validation_errors(errors)
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
            .with_context("Test context")
            .with_suggestion("Try fixing the data");

        let json = serde_json::to_string(&response).unwrap();

        // Should contain all serialized fields
        assert!(json.contains("name"));
        assert!(json.contains("message"));
        assert!(json.contains("resource"));
        assert!(json.contains("context"));
        assert!(json.contains("suggestion"));

        // Should not contain status code (marked as skip)
        assert!(!json.contains("status"));
    }

    #[test]
    fn error_response_merging_suggestion() {
        let response = ErrorResponse::BAD_REQUEST
            .with_suggestion("Check your input")
            .with_suggestion("Verify the format");

        assert_eq!(
            response.suggestion.as_deref(),
            Some("Check your input; Verify the format")
        );
    }
}
