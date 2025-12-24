use aide::axum::ApiRouter;
use axum::routing::Router;

use super::OpenApiConfig;

/// Extension trait for `axum::Router` for OpenAPI with aide.
pub trait RouterOpenApiExt<S> {
    /// Merges with OpenAPI routes.
    fn with_open_api(self, config: OpenApiConfig) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, _config: OpenApiConfig) -> Router<S> {
        // TODO: Implement aide OpenAPI integration with Scalar UI
        // For now, just return the router without OpenAPI docs
        self.into()
    }
}
