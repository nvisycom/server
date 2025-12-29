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
use aide::openapi::{Contact, License, OpenApi, Tag};
use aide::scalar::Scalar;
use aide::transform::TransformOpenApi;
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
    /// Adds OpenAPI documentation routes.
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
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S> {
        async fn serve_openapi(Extension(api): Extension<OpenApi>) -> Json<OpenApi> {
            Json(api)
        }

        let mut api = OpenApi::default();

        // Add Scalar UI route and OpenAPI JSON route
        let scalar = Scalar::new(&config.open_api_json);
        let router = self
            .route(&config.scalar_ui, scalar.axum_route())
            .route(&config.open_api_json, get(serve_openapi));

        // Generate the OpenAPI specification with tags and add it as extension
        router
            .finish_api_with(&mut api, api_docs)
            .layer(Extension(api))
    }
}

/// Transforms the OpenAPI specification with info and tags.
///
/// This function configures the OpenAPI documentation with API info and
/// organized tags for different API sections.
fn api_docs(api: TransformOpenApi) -> TransformOpenApi {
    api.title("Nvisy API")
        .summary("Document processing and annotation platform")
        .description(
            "Nvisy provides intelligent document processing, annotation, and analysis \
            capabilities. This API enables document upload, OCR processing, embedding \
            generation, and semantic search across your document collections.",
        )
        .version(env!("CARGO_PKG_VERSION"))
        .tos("https://nvisy.com/legal/terms-of-service")
        .contact(Contact {
            name: Some("Nvisy Support".to_owned()),
            url: Some("https://nvisy.com".to_owned()),
            email: Some("hello@nvisy.com".to_owned()),
            ..Contact::default()
        })
        .license(License {
            name: "Proprietary".to_owned(),
            url: Some("https://nvisy.com/license".to_owned()),
            ..License::default()
        })
        .tag(Tag {
            name: "Accounts".into(),
            description: Some("Account management and profile operations".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Authentication".into(),
            description: Some("Login, signup, and token management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Projects".into(),
            description: Some("Project creation and management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Documents".into(),
            description: Some("Document upload, processing, and retrieval".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Files".into(),
            description: Some("File upload, download, and management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Comments".into(),
            description: Some("Document and file annotations".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Members".into(),
            description: Some("Project member management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Invites".into(),
            description: Some("Project invitation handling".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Tokens".into(),
            description: Some("API token management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Templates".into(),
            description: Some("Document templates".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Pipelines".into(),
            description: Some("Processing pipelines".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Integrations".into(),
            description: Some("External service integrations".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "Webhooks".into(),
            description: Some("Webhook configuration".into()),
            ..Default::default()
        })
        .tag(Tag {
            name: "WebSocket".into(),
            description: Some("Real-time communication".into()),
            ..Default::default()
        })
}
