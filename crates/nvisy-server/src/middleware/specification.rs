//! OpenAPI specification middleware with Scalar UI integration.
//!
//! This module provides OpenAPI documentation generation and serving capabilities
//! using the [`aide`] crate with Scalar UI for interactive API exploration.
//!
//! # Overview
//!
//! The specification module offers:
//! - Automatic OpenAPI spec generation from aide's [`ApiRouter`]
//! - Scalar UI for interactive API documentation
//! - Configurable paths for JSON spec and UI endpoints
//!
//! # Usage
//!
//! ```rust
//! use aide::axum::ApiRouter;
//! use axum::Router;
//! use nvisy_server::middleware::{OpenApiConfig, RouterOpenApiExt};
//!
//! let app: Router<()> = ApiRouter::new()
//!     .with_open_api(OpenApiConfig::default());
//! ```
//!
//! [`aide`]: https://docs.rs/aide
//! [`ApiRouter`]: aide::axum::ApiRouter

use aide::axum::ApiRouter;
use aide::openapi::{Contact, Info, License, OpenApi};
use aide::scalar::Scalar;
use axum::routing::{Router, get};
use axum::{Extension, Json};
#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

/// OpenAPI configuration for aide integration.
///
/// Configures the paths where the OpenAPI JSON specification and
/// Scalar UI will be served.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
#[must_use = "config does nothing unless you use it"]
pub struct OpenApiConfig {
    /// Path which exposes the OpenAPI JSON specification.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "OPENAPI_JSON_PATH", default_value = "/api/openapi.json")
    )]
    pub open_api_json: String,

    /// Path which exposes the Scalar API reference UI.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "OPENAPI_SCALAR_PATH", default_value = "/api/scalar")
    )]
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            scalar_ui: "/api/scalar".to_owned(),
        }
    }
}

/// Extension trait for [`ApiRouter`] to add OpenAPI documentation with Scalar UI.
///
/// This trait provides convenient methods to generate and serve OpenAPI documentation
/// from your aide-annotated routes.
///
/// [`ApiRouter`]: aide::axum::ApiRouter
pub trait RouterOpenApiExt<S> {
    /// Adds OpenAPI documentation routes with default API info.
    ///
    /// This method:
    /// - Generates the OpenAPI specification from the router's API routes
    /// - Adds a route to serve the OpenAPI JSON specification
    /// - Adds a route to serve the Scalar API reference UI
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for OpenAPI and Scalar UI paths
    ///
    /// # Example
    ///
    /// ```rust
    /// use aide::axum::ApiRouter;
    /// use axum::Router;
    /// use nvisy_server::middleware::{OpenApiConfig, RouterOpenApiExt};
    ///
    /// let app: Router<()> = ApiRouter::new()
    ///     .with_open_api(OpenApiConfig::default());
    /// ```
    fn with_open_api(self, config: OpenApiConfig) -> Router<S>;

    /// Adds OpenAPI documentation routes with custom OpenAPI info.
    ///
    /// Use this method when you need full control over the OpenAPI [`Info`] object,
    /// including title, description, contact information, and license.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for OpenAPI and Scalar UI paths
    /// * `info` - Custom OpenAPI info metadata
    ///
    /// [`Info`]: aide::openapi::Info
    fn with_open_api_info(self, config: OpenApiConfig, info: Info) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: OpenApiConfig) -> Router<S> {
        let info = Info {
            title: "Nvisy API".to_owned(),
            summary: Some("Document processing and annotation platform".to_owned()),
            description: Some(
                "Nvisy provides intelligent document processing, annotation, and analysis \
                capabilities. This API enables document upload, OCR processing, embedding \
                generation, and semantic search across your document collections."
                    .to_owned(),
            ),
            terms_of_service: Some("https://nvisy.com/legal/terms-of-service".to_owned()),
            contact: Some(Contact {
                name: Some("Nvisy Support".to_owned()),
                url: Some("https://nvisy.com".to_owned()),
                email: Some("hello@nvisy.com".to_owned()),
                ..Contact::default()
            }),
            license: Some(License {
                name: "Proprietary".to_owned(),
                identifier: None,
                url: Some("https://nvisy.com/license".to_owned()),
                ..License::default()
            }),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Info::default()
        };

        self.with_open_api_info(config, info)
    }

    fn with_open_api_info(self, config: OpenApiConfig, info: Info) -> Router<S> {
        async fn serve_openapi(Extension(api): Extension<OpenApi>) -> Json<OpenApi> {
            Json(api)
        }

        let mut api = OpenApi {
            info,
            ..OpenApi::default()
        };

        // Add Scalar UI route and OpenAPI JSON route
        let scalar = Scalar::new(&config.open_api_json);
        let router = self
            .route(&config.scalar_ui, scalar.axum_route())
            .route(&config.open_api_json, get(serve_openapi));

        // Generate the OpenAPI specification and add it as an extension
        router.finish_api(&mut api).layer(Extension(api))
    }
}
