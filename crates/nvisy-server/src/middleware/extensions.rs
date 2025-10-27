//! Extension traits for `axum::Router` to easily apply middleware layers.

use std::time::Duration;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::middleware::from_fn;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;

use crate::middleware::error_handling::{catch_panic, handle_error};
use crate::middleware::observability::{
    create_propagate_request_id_layer, create_request_id_layer, create_sensitive_headers_layer,
    create_trace_layer, track_categorized_metrics,
};
use crate::middleware::security::{
    CorsConfig, SecurityHeadersConfig, create_body_limit_layer, create_cors_layer,
};

/// Extension trait for `axum::`[`Router`] for layering middleware.
///
/// This trait provides convenient methods to add common middleware stacks
/// to your Axum router in a composable way.
pub trait RouterExt<S> {
    /// Layers [`HandleError`], [`CatchPanic`] and [`Timeout`] middlewares.
    ///
    /// This middleware stack handles various error conditions:
    /// - Request timeouts
    /// - Panics in handlers
    /// - Tower service errors
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum duration to wait for a request to complete
    ///
    /// [`HandleError`]: axum::error_handling::HandleErrorLayer
    /// [`CatchPanic`]: tower_http::catch_panic::CatchPanicLayer
    /// [`Timeout`]: tower::timeout::TimeoutLayer
    fn with_error_handling_layer(self, timeout: Duration) -> Self;

    /// Layers [`SetRequestId`], [`Trace`] and [`PropagateRequestId`] middlewares.
    ///
    /// This middleware stack provides observability features:
    /// - Generates unique request IDs
    /// - Adds structured logging for requests
    /// - Propagates request IDs through the request lifecycle
    /// - Marks sensitive headers for redaction
    ///
    /// [`SetRequestId`]: tower_http::request_id::SetRequestIdLayer
    /// [`Trace`]: tower_http::trace::TraceLayer
    /// [`PropagateRequestId`]: tower_http::request_id::PropagateRequestIdLayer
    fn with_observability_layer(self) -> Self;

    /// Layers security middlewares including CORS, security headers, compression, and body limits.
    ///
    /// This middleware stack provides comprehensive security features:
    /// - CORS (Cross-Origin Resource Sharing) configuration
    /// - Security headers (HSTS, CSP, X-Frame-Options, etc.)
    /// - Response compression
    /// - Request body size limiting
    ///
    /// # Arguments
    ///
    /// * `cors_config` - CORS configuration
    /// * `security_config` - Security headers configuration
    fn with_security_layer(
        self,
        cors_config: CorsConfig,
        security_config: SecurityHeadersConfig,
    ) -> Self;

    /// Layers security middlewares with default configurations.
    ///
    /// This is a convenience method that uses default security settings.
    /// For production use, prefer `with_security_layer` with custom configs.
    fn with_default_security_layer(self) -> Self;

    /// Layers metrics middleware for request tracking and performance monitoring.
    ///
    /// This middleware tracks:
    /// - Request counts by category
    /// - Response times with category-specific thresholds
    /// - Request/response body sizes
    /// - Client IP addresses
    fn with_metrics_layer(self) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_error_handling_layer(self, timeout: Duration) -> Self {
        let middlewares = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_error))
            .layer(CatchPanicLayer::custom(catch_panic))
            .layer(TimeoutLayer::new(timeout));

        self.layer(middlewares)
    }

    fn with_observability_layer(self) -> Self {
        // Apply layers in reverse order (last layer wraps first)
        self.layer(create_propagate_request_id_layer())
            .layer(create_sensitive_headers_layer())
            .layer(create_trace_layer())
            .layer(create_request_id_layer())
    }

    fn with_security_layer(
        self,
        cors_config: CorsConfig,
        security_config: SecurityHeadersConfig,
    ) -> Self {
        use axum::http::header::{self, HeaderValue};
        use tower_http::set_header::SetResponseHeaderLayer;

        let cors = create_cors_layer(&cors_config);

        // Apply layers individually to avoid complex type issues
        let mut router = self
            .layer(create_body_limit_layer(16 * 1024 * 1024))
            .layer(CompressionLayer::new())
            .layer(cors)
            .layer(SetResponseHeaderLayer::overriding(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_str(&security_config.hsts_header_value()).unwrap(),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static(security_config.frame_options_value()),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::REFERRER_POLICY,
                HeaderValue::from_static(security_config.referrer_policy_value()),
            ));

        // Add CSP if configured
        if let Some(csp) = security_config.csp_header_value() {
            router = router.layer(SetResponseHeaderLayer::overriding(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(csp).unwrap(),
            ));
        }

        router
    }

    fn with_default_security_layer(self) -> Self {
        let cors_config = CorsConfig::default();
        let security_config = SecurityHeadersConfig::default();
        self.with_security_layer(cors_config, security_config)
    }

    fn with_metrics_layer(self) -> Self {
        let metrics = from_fn(track_categorized_metrics);
        let middlewares = ServiceBuilder::new().layer(metrics);
        self.layer(middlewares)
    }
}
