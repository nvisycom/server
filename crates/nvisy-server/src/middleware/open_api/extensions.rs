use axum::routing::Router;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use super::OpenApiConfig;

/// Generates the OpenApi object.
#[derive(Debug, OpenApi)]
struct ApiDoc;

/// Extension trait for `axum::`[`Router`] for [`OpenApi`](utoipa::OpenApi).
pub trait RouterOpenApiExt<S> {
    /// Merges with [`OpenApi`](utoipa::OpenApi) routes.
    fn with_open_api(self, config: OpenApiConfig) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: OpenApiConfig) -> Router<S> {
        let (router, open_api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
            .merge(self)
            .split_for_parts();

        router
            .merge(SwaggerUi::new(config.swagger_ui).url(config.open_api_json, open_api.clone()))
            .merge(Scalar::with_url(config.scalar_ui, open_api))
    }
}
