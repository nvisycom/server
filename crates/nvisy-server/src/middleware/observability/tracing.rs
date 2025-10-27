//! Request tracing and logging middleware.

use axum::http::header;
use tower_http::request_id::MakeRequestUuid;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;

/// Creates request ID maker for generating unique request IDs.
pub fn create_request_id_layer() -> tower_http::request_id::SetRequestIdLayer<MakeRequestUuid> {
    tower_http::request_id::SetRequestIdLayer::new(
        header::HeaderName::from_static("x-request-id"),
        MakeRequestUuid,
    )
}

/// Creates trace layer for HTTP logging.
pub fn create_trace_layer()
-> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}

/// Creates sensitive headers layer to redact auth info from logs.
pub fn create_sensitive_headers_layer() -> SetSensitiveRequestHeadersLayer {
    SetSensitiveRequestHeadersLayer::new([header::AUTHORIZATION, header::COOKIE])
}

/// Creates request ID propagation layer.
pub fn create_propagate_request_id_layer() -> tower_http::request_id::PropagateRequestIdLayer {
    tower_http::request_id::PropagateRequestIdLayer::new(header::HeaderName::from_static(
        "x-request-id",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_request_id_layer() {
        let _layer = create_request_id_layer();
        // Layer creation should not panic
    }

    #[test]
    fn test_create_trace_layer() {
        let _layer = create_trace_layer();
        // Layer creation should not panic
    }

    #[test]
    fn test_create_sensitive_headers_layer() {
        let _layer = create_sensitive_headers_layer();
        // Layer creation should not panic
    }

    #[test]
    fn test_create_propagate_request_id_layer() {
        let _layer = create_propagate_request_id_layer();
        // Layer creation should not panic
    }
}
