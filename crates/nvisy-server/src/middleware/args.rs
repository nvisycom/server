//! Aggregated HTTP-middleware configuration, shared by the first-party CLI and
//! any downstream binary that embeds this server.
//!
//! [`MiddlewareArgs`] groups the middleware configs a binary applies to the
//! router (CORS, OpenAPI, recovery) into one `clap::Args` group, so a wrapping
//! binary can `#[clap(flatten)]` it instead of re-declaring each config. The
//! request-limit [`UploadConfig`](crate::middleware::UploadConfig) is part of
//! [`ServiceArgs`](crate::ServiceArgs) instead, since the running state also
//! needs it.
//!
//! [`RouterMiddlewareExt::with_middleware`] applies the full standard stack in
//! one call, so a binary need not re-chain `with_open_api` → `with_metrics` →
//! `with_security` → `with_observability` → `with_recovery` by hand.

use aide::axum::ApiRouter;
use axum::Router;

use crate::args::TRACING_TARGET_CONFIG;
use crate::middleware::{
    CorsConfig, OpenApiConfig, RecoveryConfig, RouterObservabilityExt, RouterOpenApiExt,
    RouterRecoveryExt, RouterSecurityExt, SecurityHeadersConfig, UploadConfig,
};

/// The HTTP-middleware configs applied to the router.
///
/// The `clap::Args` derive is gated on the `cli` feature.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct MiddlewareArgs {
    /// CORS (Cross-Origin Resource Sharing) configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub cors: CorsConfig,

    /// OpenAPI documentation configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub openapi: OpenApiConfig,

    /// Recovery (timeout/panic-handling) middleware configuration.
    #[cfg_attr(feature = "cli", clap(flatten))]
    pub recovery: RecoveryConfig,
}

impl MiddlewareArgs {
    /// Logs the middleware configuration at info level.
    ///
    /// A binary calls this to echo its resolved middleware settings at startup; a
    /// downstream that flattens `MiddlewareArgs` gets the same echo without
    /// re-implementing it.
    pub fn log(&self) {
        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            origins = ?self.cors.allowed_origins,
            credentials = self.cors.allow_credentials,
            "CORS configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            openapi_path = %self.openapi.open_api_json,
            scalar_path = %self.openapi.scalar_ui,
            "OpenAPI configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CONFIG,
            request_timeout = ?self.recovery.request_timeout,
            "Recovery configuration"
        );
    }
}

/// Applies the standard HTTP-middleware stack to a router in one call.
pub trait RouterMiddlewareExt<S> {
    /// Serves OpenAPI docs, then layers metrics, security headers + CORS + body
    /// limits, observability, and recovery — the full standard stack, in the
    /// order the server applies them.
    ///
    /// `upload` supplies the request-body limits (it lives on
    /// [`ServiceArgs`](crate::ServiceArgs), so pass `service.upload`). Security
    /// headers use their defaults; a binary needing custom headers should apply
    /// the individual `with_*` layers itself.
    fn with_middleware(self, middleware: &MiddlewareArgs, upload: &UploadConfig) -> Router<S>;
}

impl<S> RouterMiddlewareExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_middleware(self, middleware: &MiddlewareArgs, upload: &UploadConfig) -> Router<S> {
        self.with_open_api(&middleware.openapi)
            .with_metrics()
            .with_security(&middleware.cors, upload, &SecurityHeadersConfig::default())
            .with_observability()
            .with_recovery(&middleware.recovery)
    }
}
