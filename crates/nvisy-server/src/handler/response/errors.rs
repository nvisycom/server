use std::borrow::Cow;
use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use schemars::JsonSchema;
use serde::Serialize;
use validator::ValidationErrors;

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
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse<'a> {
    /// The error name/type identifier
    pub name: Cow<'a, str>,
    /// User-friendly error message safe for client display
    pub message: Cow<'a, str>,
    /// The resource that the error relates to (optional, set by handler)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Cow<'a, str>>,
    /// Helpful suggestion for resolving the error (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Cow<'a, str>>,
    /// Validation error details for field-specific errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<Vec<ValidationErrorDetail>>,

    /// Error correlation ID for tracking
    #[serde(skip)]
    pub correlation_id: Option<Cow<'a, str>>,
    /// Internal context for debugging (optional, not exposed to client)
    #[serde(skip)]
    pub context: Option<Cow<'a, str>>,
    /// HTTP status code (not serialized in JSON)
    #[serde(skip)]
    pub status: StatusCode,
}

impl<'a> ErrorResponse<'a> {
    // 4xx Client Errors
    pub const BAD_REQUEST: Self = Self::new(
        "bad_request",
        "Invalid request data.",
        StatusCode::BAD_REQUEST,
    );
    pub const CONFLICT: Self =
        Self::new("conflict", "Resource state conflict.", StatusCode::CONFLICT);
    pub const FORBIDDEN: Self = Self::new("forbidden", "Access denied.", StatusCode::FORBIDDEN);
    pub const GATEWAY_TIMEOUT: Self = Self::new(
        "gateway_timeout",
        "Request timed out.",
        StatusCode::GATEWAY_TIMEOUT,
    );
    // 5xx Server Errors
    pub const INTERNAL_SERVER_ERROR: Self = Self::new(
        "internal_server_error",
        "Internal server error.",
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    // Authentication Errors
    pub const MALFORMED_AUTH_TOKEN: Self = Self::new(
        "malformed_auth_token",
        "Malformed auth token.",
        StatusCode::UNAUTHORIZED,
    );
    pub const MISSING_AUTH_TOKEN: Self = Self::new(
        "missing_auth_token",
        "Missing auth token.",
        StatusCode::UNAUTHORIZED,
    );
    pub const MISSING_PATH_PARAM: Self = Self::new(
        "missing_path_param",
        "Missing path parameter.",
        StatusCode::BAD_REQUEST,
    );
    pub const NOT_FOUND: Self =
        Self::new("not_found", "Resource not found.", StatusCode::NOT_FOUND);
    pub const NOT_IMPLEMENTED: Self = Self::new(
        "not_implemented",
        "Not implemented.",
        StatusCode::NOT_IMPLEMENTED,
    );
    pub const PAYLOAD_TOO_LARGE: Self = Self::new(
        "payload_too_large",
        "Payload too large.",
        StatusCode::PAYLOAD_TOO_LARGE,
    );
    pub const SERVICE_UNAVAILABLE: Self = Self::new(
        "service_unavailable",
        "Service unavailable.",
        StatusCode::SERVICE_UNAVAILABLE,
    );
    pub const TOKEN_EXPIRED: Self =
        Self::new("token_expired", "Token expired.", StatusCode::UNAUTHORIZED);
    pub const TOO_MANY_REQUESTS: Self = Self::new(
        "too_many_requests",
        "Rate limit exceeded.",
        StatusCode::TOO_MANY_REQUESTS,
    );
    pub const UNAUTHORIZED: Self = Self::new(
        "unauthorized",
        "Invalid credentials.",
        StatusCode::UNAUTHORIZED,
    );
    pub const UNSUPPORTED_MEDIA_TYPE: Self = Self::new(
        "unsupported_media_type",
        "Unsupported media type.",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
    );
    pub const VALIDATION_ERROR: Self = Self::new(
        "validation_error",
        "Validation failed.",
        StatusCode::BAD_REQUEST,
    );

    /// Creates a new error response.
    #[inline]
    pub const fn new(name: &'a str, message: &'a str, status: StatusCode) -> Self {
        Self {
            name: Cow::Borrowed(name),
            message: Cow::Borrowed(message),
            resource: None,
            context: None,
            suggestion: None,
            validation: None,
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
        let base = self.message.trim_end_matches('.');
        self.message = Cow::Owned(format!("{}. {}", base, new_message));
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
        self.validation = Some(errors);
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
    fn into_response(self) -> Response {
        tracing::warn!(
            status = %self.status,
            name = %self.name,
            message = %self.message,
            resource = ?self.resource,
            context = ?self.context,
            "HTTP error response"
        );
        (self.status, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_merging_resource() {
        let response = ErrorResponse::NOT_FOUND
            .with_resource("workspace")
            .with_resource("document");

        assert_eq!(response.resource.as_deref(), Some("workspace/document"));
    }

    #[test]
    fn error_response_merging_message() {
        let response = ErrorResponse::BAD_REQUEST
            .with_message("Invalid format")
            .with_message("Missing required field");

        assert_eq!(
            &response.message,
            "Invalid request data. Invalid format. Missing required field"
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
        assert!(json.contains("suggestion"));

        // Should not contain skipped fields
        assert!(!json.contains("context"));
        assert!(!json.contains("status"));
        assert!(!json.contains("correlationId"));
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
