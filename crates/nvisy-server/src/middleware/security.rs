//! Security middleware for HTTP request protection.
//!
//! This module provides comprehensive security middleware including CORS
//! configuration, security headers, request body size limiting, and response
//! compression. The security stack protects against common web vulnerabilities
//! such as XSS, clickjacking, protocol downgrade attacks, and request smuggling.

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::Method;
use axum::http::header::{self, HeaderValue};
#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::utility::{DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE};

/// Extension trait for `axum::`[`Router`] to apply security middleware.
///
/// This trait provides convenient methods to add comprehensive security
/// middleware including CORS, security headers, compression, and body limits.
pub trait RouterSecurityExt<S> {
    /// Layers security middlewares with the provided configurations.
    ///
    /// This middleware stack applies CORS rules, security headers including
    /// HSTS and CSP, response compression, and request body size limits.
    fn with_security(self, cors: &CorsConfig, headers: &SecurityHeadersConfig) -> Self;

    /// Layers security middlewares with default configurations.
    ///
    /// Uses development-friendly CORS settings and production-ready security
    /// headers. For production deployments, prefer `with_security` with
    /// explicit configuration.
    fn with_default_security(self) -> Self;
}

impl<S> RouterSecurityExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_security(self, cors: &CorsConfig, headers: &SecurityHeadersConfig) -> Self {
        let cors_layer = CorsLayer::new()
            .allow_origin(cors.to_header_values())
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
            .expose_headers([header::AUTHORIZATION])
            .allow_credentials(cors.allow_credentials)
            .max_age(cors.max_age());

        let mut router = self
            .layer(DefaultBodyLimit::max(DEFAULT_MAX_BODY_SIZE))
            .layer(RequestBodyLimitLayer::new(DEFAULT_MAX_FILE_BODY_SIZE))
            .layer(CompressionLayer::new())
            .layer(cors_layer)
            .layer(SetResponseHeaderLayer::overriding(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_str(&headers.hsts_header_value()).unwrap(),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static(headers.frame_options.as_str()),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::REFERRER_POLICY,
                HeaderValue::from_static(headers.referrer_policy.as_str()),
            ));

        if let Some(csp) = headers.content_security_policy.as_deref() {
            router = router.layer(SetResponseHeaderLayer::overriding(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(csp).unwrap(),
            ));
        }

        router
    }

    fn with_default_security(self) -> Self {
        self.with_security(&CorsConfig::default(), &SecurityHeadersConfig::default())
    }
}

/// CORS (Cross-Origin Resource Sharing) configuration.
///
/// Controls which origins can access your API and what HTTP methods
/// and headers are allowed in cross-origin requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
#[must_use = "config does nothing unless you use it"]
pub struct CorsConfig {
    /// List of allowed CORS origins.
    ///
    /// If empty, defaults to localhost origins for development.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "CORS_ORIGINS", value_delimiter = ',')
    )]
    pub allowed_origins: Vec<String>,

    /// Maximum age for CORS preflight requests in seconds.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "CORS_MAX_AGE", default_value = "3600")
    )]
    pub max_age_seconds: u64,

    /// Whether to allow credentials in CORS requests.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "CORS_ALLOW_CREDENTIALS", default_value = "true")
    )]
    pub allow_credentials: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(),
            max_age_seconds: 3600,
            allow_credentials: true,
        }
    }
}

impl CorsConfig {
    /// Returns the CORS max age as a Duration.
    pub fn max_age(&self) -> Duration {
        Duration::from_secs(self.max_age_seconds)
    }

    /// Converts configured origins to HeaderValue list, falling back to localhost for development.
    pub fn to_header_values(&self) -> Vec<HeaderValue> {
        if self.allowed_origins.is_empty() {
            vec![
                "http://localhost:3000".parse().unwrap(),
                "http://localhost:8080".parse().unwrap(),
                "http://127.0.0.1:3000".parse().unwrap(),
                "http://127.0.0.1:8080".parse().unwrap(),
                "http://localhost:5173".parse().unwrap(),
            ]
        } else {
            self.allowed_origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect()
        }
    }
}

/// Security headers configuration for the application.
///
/// Configures various HTTP security headers that protect against
/// common web vulnerabilities including XSS, clickjacking, and MITM attacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct SecurityHeadersConfig {
    /// HSTS max age in seconds. Forces browsers to use HTTPS for this duration.
    pub hsts_max_age_seconds: u64,

    /// Whether to include subdomains in HSTS policy.
    pub hsts_include_subdomains: bool,

    /// Content Security Policy directives controlling resource loading.
    pub content_security_policy: Option<String>,

    /// X-Frame-Options value protecting against clickjacking.
    pub frame_options: FrameOptions,

    /// Referrer-Policy controlling referrer information in requests.
    pub referrer_policy: ReferrerPolicy,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            hsts_max_age_seconds: 31_536_000,
            hsts_include_subdomains: true,
            content_security_policy: Some(
                "default-src 'self'; \
                 script-src 'self' 'unsafe-inline'; \
                 style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data:; \
                 connect-src 'self'; \
                 frame-ancestors 'none'; \
                 base-uri 'self'; \
                 form-action 'self'"
                    .to_owned(),
            ),
            frame_options: FrameOptions::Deny,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
        }
    }
}

impl SecurityHeadersConfig {
    /// Returns the HSTS header value as a string.
    pub fn hsts_header_value(&self) -> String {
        if self.hsts_include_subdomains {
            format!("max-age={}; includeSubDomains", self.hsts_max_age_seconds)
        } else {
            format!("max-age={}", self.hsts_max_age_seconds)
        }
    }
}

/// X-Frame-Options header values controlling frame embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameOptions {
    /// The page cannot be displayed in a frame, regardless of the site.
    Deny,
    /// The page can only be displayed in a frame on the same origin.
    SameOrigin,
}

impl FrameOptions {
    /// Returns the header value string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "DENY",
            Self::SameOrigin => "SAMEORIGIN",
        }
    }
}

/// Referrer-Policy header values controlling referrer information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferrerPolicy {
    /// No referrer information is sent.
    NoReferrer,
    /// Sends only the origin as the referrer.
    Origin,
    /// Sends full URL for same-origin, only origin for cross-origin.
    StrictOriginWhenCrossOrigin,
}

impl ReferrerPolicy {
    /// Returns the header value string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoReferrer => "no-referrer",
            Self::Origin => "origin",
            Self::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
        }
    }
}
