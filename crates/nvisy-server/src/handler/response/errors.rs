use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use schemars::JsonSchema;
use serde::Serialize;

/// The serialized shape of an HTTP error: the inert wire/OpenAPI-schema view
/// that [`Error`](crate::handler::Error) renders to at the response boundary.
///
/// It carries no builder logic — [`Error`](crate::handler::Error) is the type
/// handlers construct and thread through `Result`, and it builds an
/// `ErrorResponse` directly in its `IntoResponse` impl. `context` and `status`
/// are not part of the JSON body (`context` is logged, `status` sets the HTTP
/// status line).
#[must_use = "error responses do nothing unless serialized"]
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse<'a> {
    /// The error name/type identifier.
    pub name: Cow<'a, str>,
    /// User-friendly error message safe for client display.
    pub message: Cow<'a, str>,
    /// The resource that the error relates to, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Cow<'a, str>>,

    /// Internal context for debugging; logged, never sent to the client.
    #[serde(skip)]
    pub context: Option<Cow<'a, str>>,
    /// HTTP status code; sets the response status line, not part of the body.
    #[serde(skip)]
    pub status: StatusCode,
}

impl<'a> ErrorResponse<'a> {
    /// Creates a response carrying only a kind's static defaults (name, message,
    /// status), with no per-occurrence resource or context.
    ///
    /// This is the building block for
    /// [`ErrorKind::response`](crate::handler::ErrorKind::response), the single
    /// source of truth for each kind's name, status, and message.
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
}

impl Default for ErrorResponse<'_> {
    #[inline]
    fn default() -> Self {
        crate::handler::ErrorKind::InternalServerError.response()
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
    use axum::http::StatusCode;

    use super::ErrorResponse;
    use crate::handler::ErrorKind;

    #[test]
    fn a_kinds_response_carries_its_defaults() {
        let response = ErrorKind::NotFound.response();
        assert_eq!(response.name, "not_found");
        assert_eq!(response.message, "Resource not found");
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert!(response.resource.is_none());
    }

    #[test]
    fn only_the_public_fields_serialize() {
        // `context` and `status` are `#[serde(skip)]`; the body carries just
        // name, message, and (when present) resource.
        let response = ErrorResponse {
            name: "bad_request".into(),
            message: "Test message".into(),
            resource: Some("test_resource".into()),
            context: Some("secret debugging detail".into()),
            status: StatusCode::BAD_REQUEST,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("name"));
        assert!(json.contains("message"));
        assert!(json.contains("resource"));

        assert!(!json.contains("context"));
        assert!(!json.contains("secret debugging detail"));
        assert!(!json.contains("status"));
    }
}
