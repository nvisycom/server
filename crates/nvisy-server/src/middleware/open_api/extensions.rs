use aide::axum::ApiRouter;
use aide::openapi::{Info, OpenApi};
use aide::scalar::Scalar;
use axum::routing::{Router, get};
use axum::{Extension, Json};

use super::OpenApiConfig;

/// Serves the OpenAPI JSON specification.
async fn serve_openapi(Extension(api): Extension<OpenApi>) -> Json<OpenApi> {
    Json(api)
}

/// Extension trait for `ApiRouter` to add OpenAPI documentation with Scalar UI.
pub trait RouterOpenApiExt<S> {
    /// Adds OpenAPI documentation routes with Scalar UI.
    ///
    /// This method:
    /// - Generates the OpenAPI specification from the router's API routes
    /// - Adds a route to serve the OpenAPI JSON specification
    /// - Adds a route to serve the Scalar API reference UI
    fn with_open_api(self, config: OpenApiConfig) -> Router<S>;

    /// Adds OpenAPI documentation routes with custom OpenAPI info.
    fn with_open_api_info(self, config: OpenApiConfig, info: Info) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: OpenApiConfig) -> Router<S> {
        let info = Info {
            title: "Nvisy API".to_string(),
            description: Some("Nvisy document processing and annotation API".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Info::default()
        };

        self.with_open_api_info(config, info)
    }

    fn with_open_api_info(self, config: OpenApiConfig, info: Info) -> Router<S> {
        let mut api = OpenApi {
            info,
            ..OpenApi::default()
        };

        // Add Scalar UI route and OpenAPI JSON route
        let router = self
            .route(
                &config.scalar_ui,
                Scalar::new(&config.open_api_json).axum_route(),
            )
            .route(&config.open_api_json, get(serve_openapi));

        // Generate the OpenAPI specification and add it as an extension
        router.finish_api(&mut api).layer(Extension(api))
    }
}
